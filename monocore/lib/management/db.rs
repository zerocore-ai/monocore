//! Database management for Monocore.
//!
//! This module provides database functionality for Monocore, managing both sandbox
//! and OCI (Open Container Initiative) related data. It handles database initialization,
//! migrations, and operations for storing and retrieving container images, layers,
//! and sandbox configurations.

use std::path::Path;

use chrono::{DateTime, NaiveDateTime, Utc};
use oci_spec::image::{ImageConfiguration, ImageIndex, ImageManifest, MediaType, Platform};
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use tokio::fs;

use crate::{
    models::{Config, Image, Index, Layer, Manifest, Sandbox},
    runtime::SANDBOX_STATUS_RUNNING,
    MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Migrator for the sandbox database
pub static SANDBOX_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/migrations/sandbox");

/// Migrator for the OCI database
pub static OCI_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/migrations/oci");

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initializes a new SQLite database if it doesn't already exist at the specified path.
///
/// ## Arguments
///
/// * `db_path` - Path where the SQLite database file should be created
/// * `migrator` - SQLx migrator containing database schema migrations to run
pub async fn initialize(
    db_path: impl AsRef<Path>,
    migrator: &Migrator,
) -> MonocoreResult<Pool<Sqlite>> {
    let db_path = db_path.as_ref();

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Create an empty database file if it doesn't exist
    if !db_path.exists() {
        fs::File::create(&db_path).await?;
    }

    // Create database connection pool
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    // Run migrations
    migrator.run(&pool).await?;

    Ok(pool)
}

/// Creates and returns a connection pool for SQLite database operations.
///
/// This function initializes a new SQLite connection pool with specified configuration parameters
/// for managing database connections efficiently. The pool is configured with a maximum of 5
/// concurrent connections.
pub async fn get_pool(db_path: impl AsRef<Path>) -> MonocoreResult<Pool<Sqlite>> {
    let db_path = db_path.as_ref();
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    Ok(pool)
}

/// Gets an existing database connection pool or creates a new one if the database doesn't exist.
///
/// This function combines database initialization and pool creation into a single operation.
/// If the database doesn't exist, it will be created and migrations will be run before
/// returning the connection pool.
///
/// ## Arguments
///
/// * `db_path` - Path to the SQLite database file
/// * `migrator` - SQLx migrator containing database schema migrations to run
pub async fn get_or_create_pool(
    db_path: impl AsRef<Path>,
    migrator: &Migrator,
) -> MonocoreResult<Pool<Sqlite>> {
    // Initialize the database if it doesn't exist
    initialize(&db_path, migrator).await
}

//--------------------------------------------------------------------------------------------------
// Functions: Sandboxes
//--------------------------------------------------------------------------------------------------

/// Saves or updates a sandbox in the database and returns its ID.
/// If a sandbox with the same name and config_file exists, it will be updated.
/// Otherwise, a new sandbox record will be created.
pub(crate) async fn save_or_update_sandbox(
    pool: &Pool<Sqlite>,
    name: &str,
    config_file: &str,
    config_last_modified: &DateTime<Utc>,
    status: &str,
    supervisor_pid: u32,
    microvm_pid: u32,
    rootfs_paths: &str,
    group_id: Option<u32>,
    group_ip: Option<String>,
) -> MonocoreResult<i64> {
    let sandbox = Sandbox {
        id: 0,
        name: name.to_string(),
        config_file: config_file.to_string(),
        config_last_modified: config_last_modified.clone(),
        status: status.to_string(),
        supervisor_pid,
        microvm_pid,
        rootfs_paths: rootfs_paths.to_string(),
        group_id,
        group_ip,
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    // Try to update first
    let update_result = sqlx::query(
        r#"
        UPDATE sandboxes
        SET config_last_modified = ?,
            status = ?,
            supervisor_pid = ?,
            microvm_pid = ?,
            rootfs_paths = ?,
            group_id = ?,
            group_ip = ?,
            modified_at = CURRENT_TIMESTAMP
        WHERE name = ? AND config_file = ?
        RETURNING id
        "#,
    )
    .bind(&sandbox.config_last_modified.to_rfc3339())
    .bind(&sandbox.status)
    .bind(&sandbox.supervisor_pid)
    .bind(&sandbox.microvm_pid)
    .bind(&sandbox.rootfs_paths)
    .bind(&sandbox.group_id)
    .bind(&sandbox.group_ip)
    .bind(&sandbox.name)
    .bind(&sandbox.config_file)
    .fetch_optional(pool)
    .await?;

    if let Some(record) = update_result {
        tracing::debug!("Updated existing sandbox record");
        Ok(record.get::<i64, _>("id"))
    } else {
        // If no record was updated, insert a new one
        tracing::debug!("Creating new sandbox record");
        let record = sqlx::query(
            r#"
            INSERT INTO sandboxes (
                name, config_file, config_last_modified,
                status, supervisor_pid, microvm_pid, rootfs_paths,
                group_id, group_ip
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(sandbox.name)
        .bind(sandbox.config_file)
        .bind(sandbox.config_last_modified.to_rfc3339())
        .bind(sandbox.status)
        .bind(sandbox.supervisor_pid)
        .bind(sandbox.microvm_pid)
        .bind(sandbox.rootfs_paths)
        .bind(sandbox.group_id)
        .bind(sandbox.group_ip)
        .fetch_one(pool)
        .await?;

        Ok(record.get::<i64, _>("id"))
    }
}

pub(crate) async fn get_sandbox(
    pool: &Pool<Sqlite>,
    name: &str,
    config_file: &str,
) -> MonocoreResult<Option<Sandbox>> {
    let record = sqlx::query(
        r#"
        SELECT id, name, config_file, config_last_modified, status,
               supervisor_pid, microvm_pid, rootfs_paths,
               group_id, group_ip, created_at, modified_at
        FROM sandboxes
        WHERE name = ? AND config_file = ?
        "#,
    )
    .bind(name)
    .bind(config_file)
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|row| Sandbox {
        id: row.get("id"),
        name: row.get("name"),
        config_file: row.get("config_file"),
        config_last_modified: row
            .get::<String, _>("config_last_modified")
            .parse::<DateTime<Utc>>()
            .unwrap(),
        status: row.get("status"),
        supervisor_pid: row.get("supervisor_pid"),
        microvm_pid: row.get("microvm_pid"),
        rootfs_paths: row.get("rootfs_paths"),
        group_id: row.get("group_id"),
        group_ip: row.get("group_ip"),
        created_at: parse_sqlite_datetime(&row.get::<String, _>("created_at")),
        modified_at: parse_sqlite_datetime(&row.get::<String, _>("modified_at")),
    }))
}

/// Updates the status of a sandbox identified by name and config file
pub(crate) async fn update_sandbox_status(
    pool: &Pool<Sqlite>,
    name: &str,
    config_file: &str,
    status: &str,
) -> MonocoreResult<()> {
    sqlx::query(
        r#"
        UPDATE sandboxes
        SET status = ?,
            modified_at = CURRENT_TIMESTAMP
        WHERE name = ? AND config_file = ?
        "#,
    )
    .bind(status)
    .bind(name)
    .bind(config_file)
    .execute(pool)
    .await?;

    Ok(())
}

/// Gets all sandboxes associated with a specific config file
pub(crate) async fn get_running_config_sandboxes(
    pool: &Pool<Sqlite>,
    config_file: &str,
) -> MonocoreResult<Vec<Sandbox>> {
    let records = sqlx::query(
        r#"
        SELECT id, name, config_file, config_last_modified, status,
               supervisor_pid, microvm_pid, rootfs_paths,
               group_id, group_ip, created_at, modified_at
        FROM sandboxes
        WHERE config_file = ? AND status = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(config_file)
    .bind(SANDBOX_STATUS_RUNNING)
    .fetch_all(pool)
    .await?;

    Ok(records
        .into_iter()
        .map(|row| Sandbox {
            id: row.get("id"),
            name: row.get("name"),
            config_file: row.get("config_file"),
            config_last_modified: row
                .get::<String, _>("config_last_modified")
                .parse::<DateTime<Utc>>()
                .unwrap(),
            status: row.get("status"),
            supervisor_pid: row.get("supervisor_pid"),
            microvm_pid: row.get("microvm_pid"),
            rootfs_paths: row.get("rootfs_paths"),
            group_id: row.get("group_id"),
            group_ip: row.get("group_ip"),
            created_at: parse_sqlite_datetime(&row.get::<String, _>("created_at")),
            modified_at: parse_sqlite_datetime(&row.get::<String, _>("modified_at")),
        })
        .collect())
}

//--------------------------------------------------------------------------------------------------
// Functions: Images
//--------------------------------------------------------------------------------------------------

/// Saves an image to the database and returns its ID
pub(crate) async fn save_image(
    pool: &Pool<Sqlite>,
    reference: &str,
    size_bytes: i64,
) -> MonocoreResult<i64> {
    let image = Image {
        id: 0, // Will be set by the database
        reference: reference.to_string(),
        size_bytes,
        last_used_at: Some(Utc::now()),
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    let record = sqlx::query(
        r#"
        INSERT INTO images (reference, size_bytes, last_used_at)
        VALUES (?, ?, CURRENT_TIMESTAMP)
        RETURNING id
        "#,
    )
    .bind(&image.reference)
    .bind(image.size_bytes)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("id"))
}

/// Saves an image index to the database and returns its ID
pub(crate) async fn save_index(
    pool: &Pool<Sqlite>,
    image_id: i64,
    index: &ImageIndex,
    platform: Option<&Platform>,
) -> MonocoreResult<i64> {
    let index_model = Index {
        id: 0, // Will be set by the database
        image_id,
        schema_version: index.schema_version() as i64,
        media_type: index
            .media_type()
            .as_ref()
            .map(|mt| mt.to_string())
            .unwrap_or_else(|| MediaType::ImageIndex.to_string()),
        platform_os: platform.map(|p| p.os().to_string()),
        platform_arch: platform.map(|p| p.architecture().to_string()),
        platform_variant: platform.and_then(|p| p.variant().as_ref().map(|v| v.to_string())),
        annotations_json: index
            .annotations()
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default()),
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    let record = sqlx::query(
        r#"
        INSERT INTO indexes (
            image_id, schema_version, media_type,
            platform_os, platform_arch, platform_variant,
            annotations_json
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(index_model.image_id)
    .bind(index_model.schema_version)
    .bind(&index_model.media_type)
    .bind(&index_model.platform_os)
    .bind(&index_model.platform_arch)
    .bind(&index_model.platform_variant)
    .bind(&index_model.annotations_json)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("id"))
}

/// Saves an image manifest to the database and returns its ID
pub(crate) async fn save_manifest(
    pool: &Pool<Sqlite>,
    image_id: i64,
    index_id: Option<i64>,
    manifest: &ImageManifest,
) -> MonocoreResult<i64> {
    let manifest_model = Manifest {
        id: 0, // Will be set by the database
        index_id,
        image_id,
        schema_version: manifest.schema_version() as i64,
        media_type: manifest
            .media_type()
            .as_ref()
            .map(|mt| mt.to_string())
            .unwrap_or_else(|| MediaType::ImageManifest.to_string()),
        annotations_json: manifest
            .annotations()
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default()),
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    let record = sqlx::query(
        r#"
        INSERT INTO manifests (
            index_id, image_id, schema_version,
            media_type, annotations_json
        )
        VALUES (?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(manifest_model.index_id)
    .bind(manifest_model.image_id)
    .bind(manifest_model.schema_version)
    .bind(&manifest_model.media_type)
    .bind(&manifest_model.annotations_json)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("id"))
}

/// Saves an image configuration to the database
pub(crate) async fn save_config(
    pool: &Pool<Sqlite>,
    manifest_id: i64,
    config: &ImageConfiguration,
) -> MonocoreResult<i64> {
    let config_model = Config {
        id: 0, // Will be set by the database
        manifest_id,
        media_type: MediaType::ImageConfig.to_string(),
        created: config
            .created()
            .as_ref()
            .map(|dt| dt.parse::<DateTime<Utc>>().unwrap()),
        architecture: config.architecture().to_string(),
        os: config.os().to_string(),
        os_variant: config.os_version().as_ref().map(|s| s.to_string()),
        config_env_json: config
            .config()
            .as_ref()
            .map(|c| serde_json::to_string(c.env()).unwrap_or_default()),
        config_cmd_json: config
            .config()
            .as_ref()
            .map(|c| serde_json::to_string(c.cmd()).unwrap_or_default()),
        config_working_dir: config
            .config()
            .as_ref()
            .and_then(|c| c.working_dir().as_ref().map(String::from)),
        config_entrypoint_json: config
            .config()
            .as_ref()
            .map(|c| serde_json::to_string(c.entrypoint()).unwrap_or_default()),
        config_volumes_json: config
            .config()
            .as_ref()
            .map(|c| serde_json::to_string(c.volumes()).unwrap_or_default()),
        config_exposed_ports_json: config
            .config()
            .as_ref()
            .map(|c| serde_json::to_string(c.exposed_ports()).unwrap_or_default()),
        config_user: config
            .config()
            .as_ref()
            .and_then(|c| c.user().as_ref().map(String::from)),
        rootfs_type: config.rootfs().typ().to_string(),
        rootfs_diff_ids_json: Some(
            serde_json::to_string(&config.rootfs().diff_ids()).unwrap_or_default(),
        ),
        history_json: Some(serde_json::to_string(config.history()).unwrap_or_default()),
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    let record = sqlx::query(
        r#"
        INSERT INTO configs (
            manifest_id, media_type, created, architecture,
            os, os_variant, config_env_json, config_cmd_json,
            config_working_dir, config_entrypoint_json,
            config_volumes_json, config_exposed_ports_json,
            config_user, rootfs_type, rootfs_diff_ids_json,
            history_json
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(config_model.manifest_id)
    .bind(&config_model.media_type)
    .bind(config_model.created.map(|dt| dt.to_rfc3339()))
    .bind(&config_model.architecture)
    .bind(&config_model.os)
    .bind(&config_model.os_variant)
    .bind(&config_model.config_env_json)
    .bind(&config_model.config_cmd_json)
    .bind(&config_model.config_working_dir)
    .bind(&config_model.config_entrypoint_json)
    .bind(&config_model.config_volumes_json)
    .bind(&config_model.config_exposed_ports_json)
    .bind(&config_model.config_user)
    .bind(&config_model.rootfs_type)
    .bind(&config_model.rootfs_diff_ids_json)
    .bind(&config_model.history_json)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("id"))
}

/// Saves an image layer to the database
pub(crate) async fn save_layer(
    pool: &Pool<Sqlite>,
    manifest_id: i64,
    media_type: &str,
    digest: &str,
    size_bytes: i64,
    diff_id: &str,
) -> MonocoreResult<i64> {
    let layer_model = Layer {
        id: 0, // Will be set by the database
        manifest_id,
        media_type: media_type.to_string(),
        digest: digest.to_string(),
        diff_id: diff_id.to_string(),
        size_bytes,
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    let record = sqlx::query(
        r#"
        INSERT INTO layers (
            manifest_id, media_type, digest,
            size_bytes, diff_id
        )
        VALUES (?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(layer_model.manifest_id)
    .bind(&layer_model.media_type)
    .bind(&layer_model.digest)
    .bind(layer_model.size_bytes)
    .bind(&layer_model.diff_id)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("id"))
}

/// Saves or updates an image in the database.
/// If the image exists, it updates the size_bytes and last_used_at.
/// If it doesn't exist, creates a new record.
pub(crate) async fn save_or_update_image(
    pool: &Pool<Sqlite>,
    reference: &str,
    size_bytes: i64,
) -> MonocoreResult<i64> {
    // Try to update first
    let update_result = sqlx::query(
        r#"
        UPDATE images
        SET size_bytes = ?, last_used_at = CURRENT_TIMESTAMP, modified_at = CURRENT_TIMESTAMP
        WHERE reference = ?
        RETURNING id, reference, size_bytes, last_used_at, created_at, modified_at
        "#,
    )
    .bind(size_bytes)
    .bind(reference)
    .fetch_optional(pool)
    .await?;

    if let Some(record) = update_result {
        Ok(record.get::<i64, _>("id"))
    } else {
        // If no record was updated, insert a new one
        save_image(pool, reference, size_bytes).await
    }
}

/// Saves or updates a layer in the database.
/// If the layer exists, it updates the size_bytes and other fields.
/// If it doesn't exist, creates a new record.
pub(crate) async fn save_or_update_layer(
    pool: &Pool<Sqlite>,
    manifest_id: i64,
    media_type: &str,
    digest: &str,
    size_bytes: i64,
    diff_id: &str,
) -> MonocoreResult<i64> {
    let layer_model = Layer {
        id: 0, // Will be set by the database
        manifest_id,
        media_type: media_type.to_string(),
        digest: digest.to_string(),
        diff_id: diff_id.to_string(),
        size_bytes,
        created_at: Utc::now(),
        modified_at: Utc::now(),
    };

    // Try to update first
    let update_result = sqlx::query(
        r#"
        UPDATE layers
        SET manifest_id = ?,
            media_type = ?,
            size_bytes = ?,
            diff_id = ?,
            modified_at = CURRENT_TIMESTAMP
        WHERE digest = ?
        RETURNING id
        "#,
    )
    .bind(layer_model.manifest_id)
    .bind(&layer_model.media_type)
    .bind(layer_model.size_bytes)
    .bind(&layer_model.diff_id)
    .bind(&layer_model.digest)
    .fetch_optional(pool)
    .await?;

    if let Some(record) = update_result {
        Ok(record.get::<i64, _>("id"))
    } else {
        // If no record was updated, insert a new one
        save_layer(pool, manifest_id, media_type, digest, size_bytes, diff_id).await
    }
}

/// Checks if an image exists in the database.
pub(crate) async fn image_exists(pool: &Pool<Sqlite>, reference: &str) -> MonocoreResult<bool> {
    let record = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM images
        WHERE reference = ?
        "#,
    )
    .bind(reference)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("count") > 0)
}

/// Gets all layers for an image from the database.
pub(crate) async fn get_image_layers(
    pool: &Pool<Sqlite>,
    reference: &str,
) -> MonocoreResult<Vec<Layer>> {
    let records = sqlx::query(
        r#"
        SELECT l.id, l.manifest_id, l.media_type, l.digest,
               l.diff_id, l.size_bytes, l.created_at, l.modified_at
        FROM layers l
        JOIN manifests m ON l.manifest_id = m.id
        JOIN images i ON m.image_id = i.id
        WHERE i.reference = ?
        ORDER BY l.id ASC
        "#,
    )
    .bind(reference)
    .fetch_all(pool)
    .await?;

    Ok(records
        .into_iter()
        .map(|row| Layer {
            id: row.get("id"),
            manifest_id: row.get("manifest_id"),
            media_type: row.get("media_type"),
            digest: row.get("digest"),
            diff_id: row.get("diff_id"),
            size_bytes: row.get("size_bytes"),
            created_at: parse_sqlite_datetime(&row.get::<String, _>("created_at")),
            modified_at: parse_sqlite_datetime(&row.get::<String, _>("modified_at")),
        })
        .collect())
}

/// Gets the configuration for an image from the database.
///
/// This function retrieves the configuration details for a specified image reference.
/// It includes information like architecture, OS, environment variables, command,
/// working directory, and other container configuration metadata.
///
/// ## Arguments
///
/// * `pool` - SQLite connection pool
/// * `reference` - OCI image reference string (e.g., "ubuntu:latest")
///
/// ## Returns
///
/// Returns a `MonocoreResult` containing either the image `Config` or an error
pub(crate) async fn get_image_config(
    pool: &Pool<Sqlite>,
    reference: &str,
) -> MonocoreResult<Option<Config>> {
    let record = sqlx::query(
        r#"
        SELECT c.id, c.manifest_id, c.media_type, c.created, c.architecture,
               c.os, c.os_variant, c.config_env_json, c.config_cmd_json,
               c.config_working_dir, c.config_entrypoint_json,
               c.config_volumes_json, c.config_exposed_ports_json,
               c.config_user, c.rootfs_type, c.rootfs_diff_ids_json,
               c.history_json, c.created_at, c.modified_at
        FROM configs c
        JOIN manifests m ON c.manifest_id = m.id
        JOIN images i ON m.image_id = i.id
        WHERE i.reference = ?
        LIMIT 1
        "#,
    )
    .bind(reference)
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|row| Config {
        id: row.get("id"),
        manifest_id: row.get("manifest_id"),
        media_type: row.get("media_type"),
        created: row
            .get::<Option<String>, _>("created")
            .map(|dt| dt.parse::<DateTime<Utc>>().unwrap()),
        architecture: row.get("architecture"),
        os: row.get("os"),
        os_variant: row.get("os_variant"),
        config_env_json: null_to_none(row.get("config_env_json")),
        config_cmd_json: null_to_none(row.get("config_cmd_json")),
        config_working_dir: row.get("config_working_dir"),
        config_entrypoint_json: null_to_none(row.get("config_entrypoint_json")),
        config_volumes_json: null_to_none(row.get("config_volumes_json")),
        config_exposed_ports_json: null_to_none(row.get("config_exposed_ports_json")),
        config_user: row.get("config_user"),
        rootfs_type: row.get("rootfs_type"),
        rootfs_diff_ids_json: row.get("rootfs_diff_ids_json"),
        history_json: null_to_none(row.get("history_json")),
        created_at: parse_sqlite_datetime(&row.get::<String, _>("created_at")),
        modified_at: parse_sqlite_datetime(&row.get::<String, _>("modified_at")),
    }))
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_init_sandbox_db() -> MonocoreResult<()> {
        // Create temporary directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_sandbox.db");

        // Initialize database
        initialize(&db_path, &SANDBOX_DB_MIGRATOR).await?;

        // Test database connection
        let pool = get_pool(&db_path).await?;

        // Verify tables exist by querying them
        let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&pool)
            .await?;

        let table_names: Vec<String> = tables
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();

        assert!(
            table_names.contains(&"sandboxes".to_string()),
            "sandboxes table not found"
        );
        assert!(
            table_names.contains(&"groups".to_string()),
            "groups table not found"
        );
        assert!(
            table_names.contains(&"sandbox_metrics".to_string()),
            "sandbox_metrics table not found"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_init_oci_db() -> MonocoreResult<()> {
        // Create temporary directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_oci.db");

        // Initialize database
        initialize(&db_path, &OCI_DB_MIGRATOR).await?;

        // Test database connection
        let pool = get_pool(&db_path).await?;

        // Verify tables exist by querying them
        let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&pool)
            .await?;

        let table_names: Vec<String> = tables
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();

        assert!(
            table_names.contains(&"images".to_string()),
            "images table not found"
        );
        assert!(
            table_names.contains(&"indexes".to_string()),
            "indexes table not found"
        );
        assert!(
            table_names.contains(&"manifests".to_string()),
            "manifests table not found"
        );
        assert!(
            table_names.contains(&"configs".to_string()),
            "configs table not found"
        );
        assert!(
            table_names.contains(&"layers".to_string()),
            "layers table not found"
        );

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Parses a SQLite datetime string (in "YYYY-MM-DD HH:MM:SS" format) to a DateTime<Utc>.
fn parse_sqlite_datetime(s: &str) -> DateTime<Utc> {
    let naive_dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .unwrap_or_else(|e| panic!("Failed to parse datetime string '{}': {:?}", s, e));
    DateTime::from_naive_utc_and_offset(naive_dt, Utc)
}

/// Sometimes the json columns in the database can have literal "null" values.
/// This function converts those to None.
fn null_to_none(value: Option<String>) -> Option<String> {
    value.filter(|v| v != "null")
}

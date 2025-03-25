//! Database management for Monocore.
//!
//! This module provides database functionality for Monocore, managing both sandbox
//! and OCI (Open Container Initiative) related data. It handles database initialization,
//! migrations, and operations for storing and retrieving container images, layers,
//! and sandbox configurations.

use std::path::Path;

use chrono::{DateTime, Utc};
use oci_spec::image::{ImageConfiguration, ImageIndex, ImageManifest, MediaType, Platform};
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use tokio::fs;

use crate::{runtime::SANDBOX_STATUS_RUNNING, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Migrator for the sandbox database
pub static SANDBOX_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations/sandbox");

/// Migrator for the OCI database
pub static OCI_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations/oci");

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

/// Upserts (updates or inserts) a sandbox in the database and returns its ID.
/// If a sandbox with the same name and config_file exists, it will be updated.
/// Otherwise, a new sandbox record will be created.
pub(crate) async fn upsert_sandbox(
    pool: &Pool<Sqlite>,
    name: &str,
    config_file: &str,
    config_last_modified: &DateTime<Utc>,
    status: &str,
    supervisor_pid: u32,
    microvm_pid: u32,
    rootfs_paths: &str,
) -> MonocoreResult<i64> {
    // Try to update first
    let update_result = sqlx::query(
        r#"
        UPDATE sandboxes
        SET config_last_modified = ?,
            status = ?,
            supervisor_pid = ?,
            microvm_pid = ?,
            rootfs_paths = ?,
            modified_at = CURRENT_TIMESTAMP
        WHERE name = ? AND config_file = ?
        RETURNING id
        "#,
    )
    .bind(config_last_modified.to_rfc3339())
    .bind(status)
    .bind(supervisor_pid)
    .bind(microvm_pid)
    .bind(rootfs_paths)
    .bind(name)
    .bind(config_file)
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
                status, supervisor_pid, microvm_pid, rootfs_paths
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(config_file)
        .bind(config_last_modified.to_rfc3339())
        .bind(status)
        .bind(supervisor_pid)
        .bind(microvm_pid)
        .bind(rootfs_paths)
        .fetch_one(pool)
        .await?;

        Ok(record.get::<i64, _>("id"))
    }
}

pub(crate) async fn get_sandbox(
    pool: &Pool<Sqlite>,
    name: &str,
    config_file: &str,
) -> MonocoreResult<Option<(i64, u32, u32, String, DateTime<Utc>, String)>> {
    let record = sqlx::query(
        r#"
        SELECT id, supervisor_pid, microvm_pid, rootfs_paths, config_last_modified, status
        FROM sandboxes
        WHERE name = ? AND config_file = ?
        "#,
    )
    .bind(name)
    .bind(config_file)
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|row| {
        (
            row.get::<i64, _>("id"),
            row.get::<u32, _>("supervisor_pid"),
            row.get::<u32, _>("microvm_pid"),
            row.get::<String, _>("rootfs_paths"),
            row.get::<String, _>("config_last_modified")
                .parse::<DateTime<Utc>>()
                .unwrap(),
            row.get::<String, _>("status"),
        )
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
) -> MonocoreResult<Vec<(i64, String, u32, u32, String, String)>> {
    let records = sqlx::query(
        r#"
        SELECT id, name, supervisor_pid, microvm_pid, rootfs_paths, status
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
        .map(|row| {
            (
                row.get::<i64, _>("id"),
                row.get::<String, _>("name"),
                row.get::<i64, _>("supervisor_pid") as u32,
                row.get::<i64, _>("microvm_pid") as u32,
                row.get::<String, _>("rootfs_paths"),
                row.get::<String, _>("status"),
            )
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
    let record = sqlx::query(
        r#"
        INSERT INTO images (reference, size_bytes, last_used_at)
        VALUES (?, ?, CURRENT_TIMESTAMP)
        RETURNING id
        "#,
    )
    .bind(reference)
    .bind(size_bytes)
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
    let platform_os = platform.map(|p| p.os().to_string());
    let platform_arch = platform.map(|p| p.architecture().to_string());
    let platform_variant = platform.and_then(|p| p.variant().as_ref().map(|v| v.to_string()));
    let annotations = index
        .annotations()
        .as_ref()
        .map(|a| serde_json::to_string(a).unwrap_or_default());
    let media_type = index
        .media_type()
        .as_ref()
        .map(|mt| mt.to_string())
        .unwrap_or_else(|| MediaType::ImageIndex.to_string());

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
    .bind(image_id)
    .bind(index.schema_version() as i64)
    .bind(media_type)
    .bind(platform_os)
    .bind(platform_arch)
    .bind(platform_variant)
    .bind(annotations)
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
    let annotations = manifest
        .annotations()
        .as_ref()
        .map(|a| serde_json::to_string(a).unwrap_or_default());
    let media_type = manifest
        .media_type()
        .as_ref()
        .map(|mt| mt.to_string())
        .unwrap_or_else(|| MediaType::ImageManifest.to_string());

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
    .bind(index_id)
    .bind(image_id)
    .bind(manifest.schema_version() as i64)
    .bind(media_type)
    .bind(annotations)
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
    // Convert config fields to JSON strings where needed
    let env_json = config
        .config()
        .as_ref()
        .and_then(|c| Some(serde_json::to_string(c.env()).unwrap_or_default()));
    let cmd_json = config
        .config()
        .as_ref()
        .and_then(|c| Some(serde_json::to_string(c.cmd()).unwrap_or_default()));
    let entrypoint_json = config
        .config()
        .as_ref()
        .and_then(|c| Some(serde_json::to_string(c.entrypoint()).unwrap_or_default()));
    let volumes_json = config
        .config()
        .as_ref()
        .and_then(|c| Some(serde_json::to_string(c.volumes()).unwrap_or_default()));
    let exposed_ports_json = config
        .config()
        .as_ref()
        .and_then(|c| Some(serde_json::to_string(c.exposed_ports()).unwrap_or_default()));
    let history_json = Some(serde_json::to_string(config.history()).unwrap_or_default());
    let diff_ids = Some(config.rootfs().diff_ids().join(","));
    let media_type = MediaType::ImageConfig.to_string();

    let record = sqlx::query(
        r#"
        INSERT INTO configs (
            manifest_id, media_type, created, architecture, os,
            os_variant, config_env_json, config_cmd_json,
            config_working_dir, config_entrypoint_json,
            config_volumes_json, config_exposed_ports_json, config_user,
            rootfs_type, rootfs_diff_ids_json, history_json
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(manifest_id)
    .bind(media_type)
    .bind(config.created().as_ref().map(|t| t.to_string()))
    .bind(config.architecture().to_string())
    .bind(config.os().to_string())
    .bind(config.os_version())
    .bind(env_json)
    .bind(cmd_json)
    .bind(
        config
            .config()
            .as_ref()
            .and_then(|c| c.working_dir().as_ref().map(String::from)),
    )
    .bind(entrypoint_json)
    .bind(volumes_json)
    .bind(exposed_ports_json)
    .bind(
        config
            .config()
            .as_ref()
            .and_then(|c| c.user().as_ref().map(String::from)),
    )
    .bind(config.rootfs().typ())
    .bind(diff_ids)
    .bind(history_json)
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
    .bind(manifest_id)
    .bind(media_type)
    .bind(digest)
    .bind(size_bytes)
    .bind(diff_id)
    .fetch_one(pool)
    .await?;

    Ok(record.get::<i64, _>("id"))
}

/// Saves or updates an image in the database.
/// If the image exists, it updates the size_bytes and last_used_at.
/// If it doesn't exist, creates a new record.
///
/// ## Arguments
///
/// * `pool` - The database connection pool
/// * `reference` - The reference string of the image
/// * `size_bytes` - The size of the image in bytes
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
        RETURNING id
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
///
/// ## Arguments
///
/// * `pool` - The database connection pool
/// * `manifest_id` - The ID of the manifest this layer belongs to
/// * `media_type` - The media type of the layer
/// * `digest` - The digest of the layer
/// * `size_bytes` - The size of the layer in bytes
/// * `diff_id` - The diff ID of the layer
pub(crate) async fn save_or_update_layer(
    pool: &Pool<Sqlite>,
    manifest_id: i64,
    media_type: &str,
    digest: &str,
    size_bytes: i64,
    diff_id: &str,
) -> MonocoreResult<i64> {
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
    .bind(manifest_id)
    .bind(media_type)
    .bind(size_bytes)
    .bind(diff_id)
    .bind(digest)
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
///
/// ## Arguments
///
/// * `pool` - The database connection pool
/// * `reference` - The reference string of the image to check
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
///
/// ## Arguments
///
/// * `pool` - The database connection pool
/// * `reference` - The reference string of the image to get layers for
///
/// ## Returns
///
/// A vector of tuples containing (digest, diff_id, size_bytes) for each layer
pub(crate) async fn get_image_layers(
    pool: &Pool<Sqlite>,
    reference: &str,
) -> MonocoreResult<Vec<(String, String, i64)>> {
    let records = sqlx::query(
        r#"
        SELECT l.digest, l.diff_id, l.size_bytes
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
        .map(|row| {
            (
                row.get::<String, _>("digest"),
                row.get::<String, _>("diff_id"),
                row.get::<i64, _>("size_bytes"),
            )
        })
        .collect())
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

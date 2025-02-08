use std::path::Path;

use oci_spec::image::{ImageConfiguration, ImageIndex, ImageManifest, MediaType, Platform};
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use tokio::fs;

use crate::MonocoreResult;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Migrator for the sandbox database
pub static SANDBOX_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations/sandbox");

/// Migrator for the OCI database
pub static OCI_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations/oci");

/// Migrator for the monoimage database
pub static MONOIMAGE_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations/monoimage");

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initializes a new SQLite database if it doesn't already exist at the specified path.
///
/// ## Arguments
///
/// * `db_path` - Path where the SQLite database file should be created
/// * `migrator` - SQLx migrator containing database schema migrations to run
pub async fn init_db(
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
pub async fn get_db_pool(db_path: impl AsRef<Path>) -> MonocoreResult<Pool<Sqlite>> {
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
pub async fn get_or_create_db_pool(
    db_path: impl AsRef<Path>,
    migrator: &Migrator,
) -> MonocoreResult<Pool<Sqlite>> {
    // Initialize the database if it doesn't exist
    init_db(&db_path, migrator).await
}

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
            manifest_id, media_type, full_json,
            created, architecture, os, os_variant,
            config_env_json, config_cmd_json, config_working_dir,
            config_entrypoint_json, config_volumes_json,
            config_exposed_ports_json, config_user,
            rootfs_type, rootfs_diff_ids, history_json
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(manifest_id)
    .bind(media_type)
    .bind(serde_json::to_string(config).unwrap_or_default())
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
    diff_id: Option<&str>,
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
        init_db(&db_path, &SANDBOX_DB_MIGRATOR).await?;

        // Test database connection
        let pool = get_db_pool(&db_path).await?;

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
        init_db(&db_path, &OCI_DB_MIGRATOR).await?;

        // Test database connection
        let pool = get_db_pool(&db_path).await?;

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

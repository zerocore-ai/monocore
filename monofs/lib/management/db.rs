use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;
use tokio::fs;

use crate::FsResult;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Migrator for the filesystem database
pub static FS_DB_MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations");

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initializes a new SQLite database if it doesn't already exist at the specified path.
///
/// ## Arguments
///
/// * `db_path` - Path where the SQLite database file should be created
/// * `migrator` - SQLx migrator containing database schema migrations to run
pub async fn init_db(db_path: impl AsRef<Path>, migrator: &Migrator) -> FsResult<()> {
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

    Ok(())
}

/// Creates and returns a connection pool for SQLite database operations.
///
/// This function initializes a new SQLite connection pool with specified configuration parameters
/// for managing database connections efficiently. The pool is configured with a maximum of 5
/// concurrent connections.
pub async fn get_db_pool(db_path: impl AsRef<Path>) -> FsResult<Pool<Sqlite>> {
    let db_path = db_path.as_ref();
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    Ok(pool)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_init_db() -> FsResult<()> {
        // Create temporary directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");

        // Create empty database file
        fs::File::create(&db_path).await?;

        // Initialize database
        init_db(&db_path, &FS_DB_MIGRATOR).await?;

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
            table_names.contains(&"filesystems".to_string()),
            "filesystems table not found"
        );
        assert!(
            table_names.contains(&"tags".to_string()),
            "tags table not found"
        );

        Ok(())
    }
}

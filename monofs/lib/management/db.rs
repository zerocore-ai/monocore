use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;

use crate::FsResult;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

static MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations");

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initialize the filesystem database with schema
pub async fn init_fs_db(db_path: &Path) -> FsResult<()> {
    // Create database connection pool
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    // Run migrations
    MIGRATOR.run(&pool).await?;

    Ok(())
}

/// Get a connection pool to the filesystem database
pub async fn get_fs_db_pool(db_path: &Path) -> FsResult<Pool<Sqlite>> {
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
    async fn test_init_fs_db() -> FsResult<()> {
        // Create temporary directory
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");

        // Create empty database file
        fs::File::create(&db_path).await?;

        // Initialize database
        init_fs_db(&db_path).await?;

        // Test database connection
        let pool = get_fs_db_pool(&db_path).await?;

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

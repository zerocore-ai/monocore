use std::path::Path;

use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Sqlite};
use tokio::fs;

use crate::MonocoreResult;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

static MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations");

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initialize the sandbox database with schema
pub async fn init_sandbox_db(db_path: &Path) -> MonocoreResult<()> {
    // Only initialize if database doesn't exist
    if !db_path.exists() {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Create an empty database file
        fs::File::create(&db_path).await?;

        // Create database connection pool
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await?;

        // Run migrations
        MIGRATOR.run(&pool).await?;
    }

    Ok(())
}

/// Get a connection pool to the sandbox database
pub async fn get_sandbox_db_pool(db_path: &Path) -> MonocoreResult<Pool<Sqlite>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    Ok(pool)
}

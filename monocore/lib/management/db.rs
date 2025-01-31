use std::path::Path;

use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Sqlite};
use tokio::fs;

use crate::MonocoreResult;

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
pub async fn init_db(db_path: impl AsRef<Path>, migrator: &Migrator) -> MonocoreResult<()> {
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
    init_db(&db_path, migrator).await?;

    // Create and return the connection pool
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.as_ref().display()))
        .await?;

    Ok(pool)
}

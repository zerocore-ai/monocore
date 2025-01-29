use crate::MonocoreResult;
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};

use crate::utils::path::{LOG_SUBDIR, MONOCORE_ENV_DIR, SANDBOX_DB_FILENAME};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

static MIGRATOR: Migrator = sqlx::migrate!("lib/management/migrations");

const DEFAULT_CONFIG: &str = r#"# Sandbox configurations
sandboxes: []
"#;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

// TODO: init should add .menv to .gitignore or create it if it doesn't exist
/// Initialize a new monocore environment at the specified path
pub async fn init_env(path: Option<PathBuf>) -> MonocoreResult<()> {
    // Get the target path, defaulting to current directory if none specified
    let target_path = path.unwrap_or_else(|| PathBuf::from("."));

    // Create .menv directory structure
    create_env_dirs(&target_path).await?;

    // Initialize active database
    init_active_db(&target_path).await?;

    // Create default config file if it doesn't exist
    create_default_config(&target_path).await?;

    Ok(())
}

/// Create the required directories for a monocore environment
async fn create_env_dirs(base_path: &Path) -> MonocoreResult<()> {
    let menv_path = base_path.join(MONOCORE_ENV_DIR);

    // Create main .menv directory
    fs::create_dir_all(&menv_path).await?;

    // Create log directory
    fs::create_dir_all(menv_path.join(LOG_SUBDIR)).await?;

    // We'll create rootfs directory later when monofs is ready
    // fs::create_dir_all(menv_path.join(ROOTS_SUBDIR)).await?;

    Ok(())
}

/// Initialize the active database with schema
async fn init_active_db(base_path: &Path) -> MonocoreResult<()> {
    let db_path = base_path.join(MONOCORE_ENV_DIR).join(SANDBOX_DB_FILENAME);

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

/// Create a default monocore.yaml configuration file
async fn create_default_config(base_path: &Path) -> MonocoreResult<()> {
    let config_path = base_path.join("monocore.yaml");

    // Only create if it doesn't exist
    if !config_path.exists() {
        let mut file = fs::File::create(&config_path).await?;
        file.write_all(DEFAULT_CONFIG.as_bytes()).await?;
    }

    Ok(())
}

/// Get a connection pool to the active database
pub async fn get_active_db_pool(base_path: &Path) -> MonocoreResult<Pool<Sqlite>> {
    let db_path = base_path.join(MONOCORE_ENV_DIR).join(SANDBOX_DB_FILENAME);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    Ok(pool)
}

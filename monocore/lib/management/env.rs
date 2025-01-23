use crate::MonocoreResult;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};

use crate::utils::path::{ACTIVE_DB_FILENAME, LOG_SUBDIR, MONOCORE_ENV_DIR};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const ACTIVE_DB_SCHEMA: &str = r#"
-- Create sandboxes table
CREATE TABLE IF NOT EXISTS sandboxes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    pid INTEGER,
    status TEXT NOT NULL,
    rootfs_path TEXT NOT NULL,
    group_id INTEGER,
    group_ip TEXT,
    config TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(group_id) REFERENCES groups(id)
);

-- Create groups table
CREATE TABLE IF NOT EXISTS groups (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    subnet TEXT NOT NULL,
    reach TEXT NOT NULL,
    config TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create sandbox_metrics table
CREATE TABLE IF NOT EXISTS sandbox_metrics (
    id INTEGER PRIMARY KEY,
    sandbox_id INTEGER NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    cpu_usage REAL,
    memory_usage INTEGER,
    disk_read_bytes INTEGER,
    disk_write_bytes INTEGER,
    total_disk_read INTEGER,
    total_disk_write INTEGER,
    FOREIGN KEY(sandbox_id) REFERENCES sandboxes(id)
);


-- Create indexes
CREATE INDEX IF NOT EXISTS idx_sandboxes_name ON sandboxes(name);
CREATE INDEX IF NOT EXISTS idx_sandboxes_group_id ON sandboxes(group_id);
CREATE INDEX IF NOT EXISTS idx_sandbox_metrics_sandbox_id_timestamp ON sandbox_metrics(sandbox_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_groups_name ON groups(name);
"#;

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
    let db_path = base_path.join(MONOCORE_ENV_DIR).join(ACTIVE_DB_FILENAME);

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

        // Initialize schema
        sqlx::query(ACTIVE_DB_SCHEMA).execute(&pool).await?;
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
    let db_path = base_path.join(MONOCORE_ENV_DIR).join(ACTIVE_DB_FILENAME);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
        .await?;

    Ok(pool)
}

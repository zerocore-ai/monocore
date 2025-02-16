use crate::{config::DEFAULT_CONFIG, MonocoreResult};
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};

use crate::utils::path::{LOG_SUBDIR, MONOCORE_ENV_DIR, SANDBOX_DB_FILENAME};

use super::{db, SANDBOX_DB_MIGRATOR};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initialize a new monocore environment at the specified path
///
/// ## Arguments
/// * `project_path` - Optional path where the monocore environment will be initialized. If None, uses current directory
///
/// ## Example
/// ```no_run
/// use monocore::management;
///
/// # async fn example() -> anyhow::Result<()> {
/// // Initialize in current directory
/// management::init_menv(None).await?;
///
/// // Initialize in specific directory
/// management::init_menv(Some("my_project".into())).await?;
/// # Ok(())
/// # }
/// ```
// TODO: init should add .menv to .gitignore or create it if it doesn't exist
pub async fn init_menv(project_path: Option<PathBuf>) -> MonocoreResult<()> {
    // Get the target path, defaulting to current directory if none specified
    let project_path = project_path.unwrap_or_else(|| PathBuf::from("."));

    // Get the .menv directory path
    let menv_path = project_path.join(MONOCORE_ENV_DIR);
    fs::create_dir_all(&menv_path).await?;

    // Create .menv directory structure
    create_menv_dirs(&menv_path).await?;

    // Get the sandbox database path
    let db_path = menv_path.join(SANDBOX_DB_FILENAME);

    // Initialize sandbox database
    let _ = db::init_db(&db_path, &SANDBOX_DB_MIGRATOR).await?;
    tracing::info!("initialized sandbox database at {}", db_path.display());

    // Create default config file if it doesn't exist
    create_default_config(&project_path).await?;
    tracing::info!(
        "created default config file at {}",
        project_path.join("monocore.yaml").display()
    );

    Ok(())
}

/// Create the required directories for a monocore environment
async fn create_menv_dirs(menv_path: &Path) -> MonocoreResult<()> {
    // Create log directory if it doesn't exist
    fs::create_dir_all(menv_path.join(LOG_SUBDIR)).await?;

    // We'll create rootfs directory later when monofs is ready
    // fs::create_dir_all(menv_path.join(ROOTS_SUBDIR)).await?;

    Ok(())
}

/// Create a default monocore.yaml configuration file
async fn create_default_config(project_path: &Path) -> MonocoreResult<()> {
    let config_path = project_path.join("monocore.yaml");

    // Only create if it doesn't exist
    if !config_path.exists() {
        let mut file = fs::File::create(&config_path).await?;
        file.write_all(DEFAULT_CONFIG.as_bytes()).await?;
    }

    Ok(())
}

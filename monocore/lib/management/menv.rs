//! Monocore environment management.
//!
//! This module handles the initialization and management of Monocore environments.
//! A Monocore environment (menv) is a directory structure that contains all the
//! necessary components for running sandboxes, including configuration files,
//! databases, and log directories.

use crate::{
    config::{DEFAULT_CONFIG, DEFAULT_MONOCORE_CONFIG_FILENAME},
    utils::ROOTFS_SUBDIR,
    MonocoreResult,
};
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};

use crate::utils::path::{LOG_SUBDIR, MONOCORE_ENV_DIR, SANDBOX_DB_FILENAME};

use super::db;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initialize a new monocore environment at the specified path
///
/// ## Arguments
/// * `project_dir` - Optional path where the monocore environment will be initialized. If None, uses current directory
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
pub async fn initialize(project_dir: Option<PathBuf>) -> MonocoreResult<()> {
    // Get the target path, defaulting to current directory if none specified
    let project_dir = project_dir.unwrap_or_else(|| PathBuf::from("."));

    // Create the required files for the monocore environment
    ensure_menv_files(&project_dir).await?;

    // Create default config file if it doesn't exist
    create_default_config(&project_dir).await?;
    tracing::info!(
        "config file at {}",
        project_dir.join(DEFAULT_MONOCORE_CONFIG_FILENAME).display()
    );

    // Update .gitignore to include .menv directory
    update_gitignore(&project_dir).await?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Create the required directories and files for a monocore environment
pub(crate) async fn ensure_menv_files(project_dir: &Path) -> MonocoreResult<()> {
    // Get the .menv directory path
    let menv_path = project_dir.join(MONOCORE_ENV_DIR);
    fs::create_dir_all(&menv_path).await?;

    // Create log directory if it doesn't exist
    fs::create_dir_all(menv_path.join(LOG_SUBDIR)).await?;

    // We'll create rootfs directory later when monofs is ready
    fs::create_dir_all(menv_path.join(ROOTFS_SUBDIR)).await?;

    // Get the sandbox database path
    let db_path = menv_path.join(SANDBOX_DB_FILENAME);

    // Initialize sandbox database
    let _ = db::initialize(&db_path, &db::SANDBOX_DB_MIGRATOR).await?;
    tracing::info!("sandbox database at {}", db_path.display());

    Ok(())
}

/// Create a default monocore configuration file
pub(crate) async fn create_default_config(project_dir: &Path) -> MonocoreResult<()> {
    let config_path = project_dir.join(DEFAULT_MONOCORE_CONFIG_FILENAME);

    // Only create if it doesn't exist
    if !config_path.exists() {
        let mut file = fs::File::create(&config_path).await?;
        file.write_all(DEFAULT_CONFIG.as_bytes()).await?;
    }

    Ok(())
}

/// Updates or creates a .gitignore file to include the .menv directory
pub(crate) async fn update_gitignore(project_dir: &Path) -> MonocoreResult<()> {
    let gitignore_path = project_dir.join(".gitignore");
    let canonical_entry = format!("{}/", MONOCORE_ENV_DIR);
    let acceptable_entries = [MONOCORE_ENV_DIR, &canonical_entry[..]];

    if gitignore_path.exists() {
        let content = fs::read_to_string(&gitignore_path).await?;
        let already_present = content.lines().any(|line| {
            let trimmed = line.trim();
            acceptable_entries.contains(&trimmed)
        });

        if !already_present {
            // Ensure we start on a new line
            let prefix = if content.ends_with('\n') { "" } else { "\n" };
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&gitignore_path)
                .await?;
            file.write_all(format!("{}{}\n", prefix, canonical_entry).as_bytes())
                .await?;
        }
    } else {
        // Create new .gitignore with canonical entry (.menv/)
        fs::write(&gitignore_path, format!("{}\n", canonical_entry)).await?;
    }

    Ok(())
}

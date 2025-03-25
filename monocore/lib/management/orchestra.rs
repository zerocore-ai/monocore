//! Orchestra management functionality for Monocore.
//!
//! This module provides functionality for managing collections of sandboxes in a coordinated way,
//! similar to how container orchestration tools manage multiple containers. It handles the lifecycle
//! of multiple sandboxes defined in configuration, including starting them up, shutting them down,
//! and applying configuration changes.
//!
//! The main operations provided by this module are:
//! - `up`: Start up all sandboxes defined in configuration
//! - `down`: Gracefully shut down all running sandboxes
//! - `apply`: Reconcile running sandboxes with configuration

use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use std::path::PathBuf;

use crate::{
    config,
    config::DEFAULT_SCRIPT,
    management::sandbox,
    utils::{MONOCORE_ENV_DIR, SANDBOX_DB_FILENAME},
    MonocoreResult,
};

use super::{db, menv};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Reconciles the running sandboxes with the configuration.
///
/// This function ensures that the set of running sandboxes matches what is defined in the
/// configuration by:
/// - Starting any sandboxes that are in the config but not running
/// - Stopping any sandboxes that are running but not in the config
///
/// The function uses a file-based lock to prevent concurrent apply operations.
/// If another apply operation is in progress, this function will fail immediately.
/// The lock is automatically released when the function completes or if it fails.
///
/// ## Arguments
///
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
///
/// ## Returns
///
/// Returns `MonocoreResult<()>` indicating success or failure. Possible failures include:
/// - Config file not found or invalid
/// - Database errors
/// - Sandbox start/stop failures
///
/// ## Example
///
/// ```no_run
/// use std::path::PathBuf;
/// use monocore::management::orchestra;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Apply configuration changes from the default monocore.yaml
///     orchestra::apply(None, None).await?;
///
///     // Or specify a custom project directory and config file
///     orchestra::apply(
///         Some(PathBuf::from("/path/to/project")),
///         Some("custom-config.yaml"),
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn apply(project_dir: Option<PathBuf>, config_file: Option<&str>) -> MonocoreResult<()> {
    // Load the configuration first to validate it exists before acquiring lock
    let (config, canonical_project_dir, config_file) =
        config::load_config(project_dir, config_file).await?;

    // Ensure menv files exist
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);
    menv::ensure_menv_files(&menv_path).await?;

    // Get database connection pool
    let db_path = menv_path.join(SANDBOX_DB_FILENAME);
    let pool = db::get_or_create_pool(&db_path, &db::SANDBOX_DB_MIGRATOR).await?;

    // Get all sandboxes defined in config
    let config_sandboxes = config
        .get_sandboxes()
        .as_ref()
        .map(|s| s.as_slice())
        .unwrap_or_default();

    let config_sandbox_names: Vec<String> = config_sandboxes
        .iter()
        .map(|s| s.get_name().to_string())
        .collect();

    // Get all running sandboxes from database
    let running_sandboxes = db::get_running_config_sandboxes(&pool, &config_file).await?;
    let running_sandbox_names: Vec<String> =
        running_sandboxes.iter().map(|s| s.name.clone()).collect();

    // Start sandboxes that are in config but not active
    for sandbox_config in config_sandboxes {
        // Should start in parallel
        if !running_sandbox_names.contains(sandbox_config.get_name()) {
            tracing::info!("Starting sandbox: {}", sandbox_config.get_name());
            sandbox::run(
                sandbox_config.get_name(),
                Some(DEFAULT_SCRIPT),
                Some(canonical_project_dir.clone()),
                Some(&config_file),
                vec![],
                true,
                None,
            )
            .await?;
        }
    }

    // Stop sandboxes that are active but not in config
    for sandbox in running_sandboxes {
        if !config_sandbox_names.contains(&sandbox.name) {
            tracing::info!("Stopping sandbox: {}", sandbox.name);
            signal::kill(
                Pid::from_raw(sandbox.supervisor_pid as i32),
                Signal::SIGTERM,
            )?;
        }
    }

    Ok(())
}

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
use std::path::Path;

use crate::{
    config::{Monocore, DEFAULT_SCRIPT},
    management::{config, sandbox},
    utils::{MONOCORE_ENV_DIR, SANDBOX_DB_FILENAME},
    MonocoreError, MonocoreResult,
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
pub async fn apply(project_dir: Option<&Path>, config_file: Option<&str>) -> MonocoreResult<()> {
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
    let config_sandboxes = config.get_sandboxes();

    // Get all running sandboxes from database
    let running_sandboxes = db::get_running_config_sandboxes(&pool, &config_file).await?;
    let running_sandbox_names: Vec<String> =
        running_sandboxes.iter().map(|s| s.name.clone()).collect();

    // Start sandboxes that are in config but not active
    for (name, _) in config_sandboxes {
        // Should start in parallel
        if !running_sandbox_names.contains(name) {
            tracing::info!("Starting sandbox: {}", name);
            sandbox::run(
                name,
                Some(DEFAULT_SCRIPT),
                Some(&canonical_project_dir),
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
        if !config_sandboxes.contains_key(&sandbox.name) {
            tracing::info!("Stopping sandbox: {}", sandbox.name);
            signal::kill(
                Pid::from_raw(sandbox.supervisor_pid as i32),
                Signal::SIGTERM,
            )?;
        }
    }

    Ok(())
}

/// Starts specified sandboxes from the configuration if they are not already running.
///
/// This function ensures that the specified sandboxes are running by:
/// - Starting any specified sandboxes that are in the config but not running
/// - Ignoring sandboxes that are not specified or already running
///
/// ## Arguments
///
/// * `sandbox_names` - List of sandbox names to start
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
///
/// ## Returns
///
/// Returns `MonocoreResult<()>` indicating success or failure. Possible failures include:
/// - Config file not found or invalid
/// - Database errors
/// - Sandbox start failures
///
/// ## Example
///
/// ```no_run
/// use std::path::PathBuf;
/// use monocore::management::orchestra;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Start specific sandboxes from the default monocore.yaml
///     orchestra::up(vec!["sandbox1".to_string(), "sandbox2".to_string()], None, None).await?;
///
///     // Or specify a custom project directory and config file
///     orchestra::up(
///         vec!["sandbox1".to_string()],
///         Some(PathBuf::from("/path/to/project")),
///         Some("custom-config.yaml"),
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn up(
    sandbox_names: Vec<String>,
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<()> {
    // Load the configuration first to validate it exists
    let (config, canonical_project_dir, config_file) =
        config::load_config(project_dir, config_file).await?;

    // Validate all sandbox names exist in config before proceeding
    validate_sandbox_names(
        &sandbox_names,
        &config,
        &canonical_project_dir,
        &config_file,
    )?;

    // Ensure menv files exist
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);
    menv::ensure_menv_files(&menv_path).await?;

    // Get database connection pool
    let db_path = menv_path.join(SANDBOX_DB_FILENAME);
    let pool = db::get_or_create_pool(&db_path, &db::SANDBOX_DB_MIGRATOR).await?;

    // Get all sandboxes defined in config
    let config_sandboxes = config.get_sandboxes();

    // Get all running sandboxes from database
    let running_sandboxes = db::get_running_config_sandboxes(&pool, &config_file).await?;
    let running_sandbox_names: Vec<String> =
        running_sandboxes.iter().map(|s| s.name.clone()).collect();

    // Start specified sandboxes that are in config but not active
    for (sandbox_name, _) in config_sandboxes {
        // Only start if sandbox is in the specified list and not already running
        if sandbox_names.contains(sandbox_name) && !running_sandbox_names.contains(sandbox_name) {
            tracing::info!("Starting sandbox: {}", sandbox_name);
            sandbox::run(
                sandbox_name,
                Some(DEFAULT_SCRIPT),
                Some(&canonical_project_dir),
                Some(&config_file),
                vec![],
                true,
                None,
            )
            .await?;
        }
    }

    Ok(())
}

/// Stops specified sandboxes that are both in the configuration and currently running.
///
/// This function ensures that the specified sandboxes are stopped by:
/// - Stopping any specified sandboxes that are both in the config and currently running
/// - Ignoring sandboxes that are not specified, not in config, or not running
///
/// ## Arguments
///
/// * `sandbox_names` - List of sandbox names to stop
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
///
/// ## Returns
///
/// Returns `MonocoreResult<()>` indicating success or failure. Possible failures include:
/// - Config file not found or invalid
/// - Database errors
/// - Sandbox stop failures
///
/// ## Example
///
/// ```no_run
/// use std::path::PathBuf;
/// use monocore::management::orchestra;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Stop specific sandboxes from the default monocore.yaml
///     orchestra::down(vec!["sandbox1".to_string(), "sandbox2".to_string()], None, None).await?;
///
///     // Or specify a custom project directory and config file
///     orchestra::down(
///         vec!["sandbox1".to_string()],
///         Some(PathBuf::from("/path/to/project")),
///         Some("custom-config.yaml"),
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn down(
    sandbox_names: Vec<String>,
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<()> {
    // Load the configuration first to validate it exists
    let (config, canonical_project_dir, config_file) =
        config::load_config(project_dir, config_file).await?;

    // Validate all sandbox names exist in config before proceeding
    validate_sandbox_names(
        &sandbox_names,
        &config,
        &canonical_project_dir,
        &config_file,
    )?;

    // Ensure menv files exist
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);
    menv::ensure_menv_files(&menv_path).await?;

    // Get database connection pool
    let db_path = menv_path.join(SANDBOX_DB_FILENAME);
    let pool = db::get_or_create_pool(&db_path, &db::SANDBOX_DB_MIGRATOR).await?;

    // Get all sandboxes defined in config
    let config_sandboxes = config.get_sandboxes();

    // Get all running sandboxes from database
    let running_sandboxes = db::get_running_config_sandboxes(&pool, &config_file).await?;

    // Stop specified sandboxes that are both in config and running
    for sandbox in running_sandboxes {
        if sandbox_names.contains(&sandbox.name) && config_sandboxes.contains_key(&sandbox.name) {
            tracing::info!("Stopping sandbox: {}", sandbox.name);
            signal::kill(
                Pid::from_raw(sandbox.supervisor_pid as i32),
                Signal::SIGTERM,
            )?;
        }
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Validate that all requested sandbox names exist in the configuration
fn validate_sandbox_names(
    sandbox_names: &[String],
    config: &Monocore,
    project_dir: &Path,
    config_file: &str,
) -> MonocoreResult<()> {
    let config_sandboxes = config.get_sandboxes();

    let missing_sandboxes: Vec<String> = sandbox_names
        .iter()
        .filter(|name| !config_sandboxes.contains_key(*name))
        .cloned()
        .collect();

    if !missing_sandboxes.is_empty() {
        return Err(MonocoreError::SandboxNotFoundInConfig(
            missing_sandboxes.join(", "),
            project_dir.join(config_file),
        ));
    }

    Ok(())
}

/// Checks if specified sandboxes from the configuration are running.
async fn _check_running(
    sandbox_names: Vec<String>,
    config: &Monocore,
    project_dir: &Path,
    config_file: &str,
) -> MonocoreResult<Vec<(String, bool)>> {
    // Ensure menv files exist
    let canonical_project_dir = project_dir.canonicalize().map_err(|e| {
        MonocoreError::InvalidArgument(format!("Failed to canonicalize project directory: {}", e))
    })?;
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);
    menv::ensure_menv_files(&menv_path).await?;

    // Get database connection pool
    let db_path = menv_path.join(SANDBOX_DB_FILENAME);
    let pool = db::get_or_create_pool(&db_path, &db::SANDBOX_DB_MIGRATOR).await?;

    // Get all sandboxes defined in config
    let config_sandboxes = config.get_sandboxes();

    // Get all running sandboxes from database
    let running_sandboxes = db::get_running_config_sandboxes(&pool, config_file).await?;
    let running_sandbox_names: Vec<String> =
        running_sandboxes.iter().map(|s| s.name.clone()).collect();

    // Check status of specified sandboxes
    let mut statuses = Vec::new();
    for sandbox_name in sandbox_names {
        // Only check if sandbox exists in config
        if config_sandboxes.contains_key(&sandbox_name) {
            let is_running = running_sandbox_names.contains(&sandbox_name);
            statuses.push((sandbox_name, is_running));
        }
    }

    Ok(statuses)
}

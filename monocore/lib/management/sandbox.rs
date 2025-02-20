//! Sandbox management functionality for Monocore.
//!
//! This module provides functionality for managing sandboxes, which are isolated execution
//! environments for running applications. It handles sandbox creation, configuration,
//! and execution based on the Monocore configuration file.

use std::path::PathBuf;

use tokio::{fs, process::Command};
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{MonocoreConfig, ReferencePath, DEFAULT_MCRUN_EXE_PATH, DEFAULT_WORKDIR},
    management::rootfs,
    utils::{
        LOG_SUBDIR, MCRUN_EXE_ENV_VAR, MONOCORE_CONFIG_FILENAME, MONOCORE_ENV_DIR,
        SANDBOX_DB_FILENAME, SANDBOX_SCRIPT_DIR,
    },
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Runs a sandbox
pub async fn run_sandbox(
    name: &str,
    script: String,
    args: Vec<String>,
    project_dir: Option<PathBuf>,
    config_path: Option<PathBuf>,
) -> MonocoreResult<()> {
    // Get the target path, defaulting to current directory if none specified
    let project_dir = project_dir.unwrap_or_else(|| PathBuf::from("."));

    // Get the config file path
    let config_path =
        project_dir.join(config_path.unwrap_or_else(|| MONOCORE_CONFIG_FILENAME.into()));

    // Check if config file exists
    if !config_path.exists() {
        return Err(MonocoreError::MonocoreConfigNotFound(
            config_path.to_string_lossy().to_string(),
        ));
    }

    // Read and parse the monocore.yaml config file
    let config_contents = fs::read_to_string(&config_path).await?;
    let config: MonocoreConfig = serde_yaml::from_str(&config_contents)?;

    // Get the sandbox config
    let Some(sandbox_config) = config.get_sandbox(name) else {
        return Err(MonocoreError::SandboxNotFoundInConfig(
            name.to_string(),
            config_path,
        ));
    };

    // TODO: We should support overlayfs as default.
    let ReferencePath::Path(root_path) = sandbox_config.get_image() else {
        return Err(MonocoreError::custom(anyhow::anyhow!(
            "currently only local rootfs are supported for sandboxes, `{}` does not specify a valid rootfs",
            name
        )));
    };

    // Patch the rootfs with the sandbox scripts
    let scripts = sandbox_config.get_full_scripts();
    rootfs::patch_native_rootfs_with_scripts(root_path, scripts, sandbox_config.get_shell())?;

    // Get the .menv directory path
    let menv_path = project_dir.join(MONOCORE_ENV_DIR);
    fs::create_dir_all(&menv_path).await?;

    // Log directory
    let log_dir = menv_path.join(LOG_SUBDIR);
    fs::create_dir_all(&log_dir).await?;

    // Sandbox database path
    let sandbox_db_path = menv_path.join(SANDBOX_DB_FILENAME);

    let mcrun_path =
        monoutils::path::resolve_env_path(MCRUN_EXE_ENV_VAR, &*DEFAULT_MCRUN_EXE_PATH)?;

    // Get the workdir path
    let workdir_path = sandbox_config
        .get_workdir()
        .clone()
        .unwrap_or_else(|| Utf8UnixPathBuf::from(DEFAULT_WORKDIR));

    // Get the exec path
    let exec_path = format!("/{}/{}", SANDBOX_SCRIPT_DIR, script);

    tracing::info!("starting sandbox supervisor...");
    let mut command = Command::new(mcrun_path);
    command
        .arg("supervisor")
        .arg("--log-dir")
        .arg(&log_dir)
        .arg("--child-name")
        .arg(name)
        .arg("--sandbox-db-path")
        .arg(&sandbox_db_path)
        .arg("--root-path")
        .arg(&root_path)
        .arg("--num-vcpus")
        .arg(sandbox_config.get_cpus().to_string())
        .arg("--ram-mib")
        .arg(sandbox_config.get_ram().to_string())
        .arg("--workdir-path")
        .arg(&workdir_path)
        .arg("--exec-path")
        .arg(&exec_path);

    if !args.is_empty() {
        command.arg("--args").arg(args.join(","));
    }

    let status = command.spawn()?;

    tracing::info!(
        "started supervisor process with PID: {}",
        status.id().unwrap_or(0)
    );

    Ok(())
}

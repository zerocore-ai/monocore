//! Sandbox management functionality for Monocore.
//!
//! This module provides functionality for managing sandboxes, which are isolated execution
//! environments for running applications. It handles sandbox creation, configuration,
//! and execution based on the Monocore configuration file.

use std::path::{Path, PathBuf};

use tokio::{fs, process::Command};
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{MonocoreConfig, ReferencePath, Sandbox, DEFAULT_MCRUN_EXE_PATH, DEFAULT_MONOCORE_CONFIG_FILENAME, DEFAULT_WORKDIR},
    management::{db, image, menv, rootfs},
    oci::Reference,
    utils::{
        env, EXTRACTED_LAYER_SUFFIX, LAYERS_SUBDIR, LOG_SUBDIR, MCRUN_EXE_ENV_VAR,
        MONOCORE_ENV_DIR, OCI_DB_FILENAME, PATCH_SUBDIR, ROOTFS_SUBDIR, SANDBOX_DB_FILENAME,
        SANDBOX_SCRIPT_DIR,
    },
    vm::Rootfs,
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
    let canonical_project_dir = fs::canonicalize(project_dir).await?;

    // Get the config file path
    let full_config_path = canonical_project_dir.join(
        config_path
            .clone()
            .unwrap_or_else(|| DEFAULT_MONOCORE_CONFIG_FILENAME.into()),
    );

    tracing::debug!("full_config_path: {}", full_config_path.display());

    // Check if config file exists
    if !full_config_path.exists() {
        return Err(MonocoreError::MonocoreConfigNotFound(
            full_config_path.to_string_lossy().to_string(),
        ));
    }

    // Read and parse the monocore.yaml config file
    let config_contents = fs::read_to_string(&full_config_path).await?;
    let config: MonocoreConfig = serde_yaml::from_str(&config_contents)?;

    tracing::debug!("config: {:?}", config);

    // Get the sandbox config
    let Some(sandbox_config) = config.get_sandbox(name) else {
        return Err(MonocoreError::SandboxNotFoundInConfig(
            name.to_string(),
            full_config_path,
        ));
    };

    tracing::debug!("sandbox_config: {:?}", sandbox_config);

    // Ensure the .menv files exist
    menv::ensure_menv_files(&canonical_project_dir).await?;
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);

    let rootfs = match sandbox_config.get_image() {
        ReferencePath::Path(root_path) => {
            setup_native_rootfs(&canonical_project_dir.join(root_path), sandbox_config).await?
        }
        ReferencePath::Reference(reference) => {
            setup_image_rootfs(reference, sandbox_config, &menv_path, config_path.as_ref()).await?
        }
    };

    // Log directory
    let log_dir = menv_path.join(LOG_SUBDIR);
    fs::create_dir_all(&log_dir).await?;

    // Sandbox database path
    let sandbox_db_path = menv_path.join(SANDBOX_DB_FILENAME);

    // Get the workdir path
    let workdir_path = sandbox_config
        .get_workdir()
        .clone()
        .unwrap_or_else(|| Utf8UnixPathBuf::from(DEFAULT_WORKDIR));

    // Get the exec path
    let exec_path = format!("/{}/{}", SANDBOX_SCRIPT_DIR, script);

    tracing::info!("starting sandbox supervisor...");

    tracing::debug!("rootfs: {:?}", rootfs);
    tracing::debug!("workdir_path: {}", workdir_path);
    tracing::debug!("exec_path: {}", exec_path);

    let mcrun_path =
        monoutils::path::resolve_env_path(MCRUN_EXE_ENV_VAR, &*DEFAULT_MCRUN_EXE_PATH)?;

    let mut command = Command::new(mcrun_path);
    command
        .arg("supervisor")
        .arg("--log-dir")
        .arg(&log_dir)
        .arg("--child-name")
        .arg(name)
        .arg("--sandbox-db-path")
        .arg(&sandbox_db_path)
        .arg("--num-vcpus")
        .arg(sandbox_config.get_cpus().to_string())
        .arg("--ram-mib")
        .arg(sandbox_config.get_ram().to_string())
        .arg("--workdir-path")
        .arg(&workdir_path)
        .arg("--exec-path")
        .arg(&exec_path)
        .arg("--forward-output");

    // Pass the rootfs
    match rootfs {
        Rootfs::Native(path) => {
            command.arg("--native-rootfs").arg(path);
        }
        Rootfs::Overlayfs(paths) => {
            for path in paths {
                command.arg("--overlayfs-layer").arg(path);
            }
        }
    }

    // Only pass RUST_LOG if it's set in the environment
    if let Some(rust_log) = std::env::var_os("RUST_LOG") {
        tracing::debug!("using existing RUST_LOG: {:?}", rust_log);
        command.env("RUST_LOG", rust_log);
    }

    // Pass the extra arguments last.
    if !args.is_empty() {
        command.arg("--");
        for arg in args {
            command.arg(arg);
        }
    }

    let mut child = command.spawn()?;

    tracing::info!(
        "started supervisor process with PID: {}",
        child.id().unwrap_or(0)
    );

    // Wait for the child process to complete
    let status = child.wait().await?;
    if !status.success() {
        tracing::error!("child process exited with status: {}", status);
        return Err(MonocoreError::SupervisorError(format!(
            "child process failed with exit status: {}",
            status
        )));
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

async fn setup_image_rootfs(
    image: &Reference,
    sandbox_config: &Sandbox,
    menv_path: &Path,
    config_path: Option<&PathBuf>,
) -> MonocoreResult<Rootfs> {
    // Pull the image from the registry
    tracing::info!("pulling image: {}", image);
    image::pull_image(image.clone(), true, false, None).await?;

    // Get the monocore home path and database path
    let monocore_home_path = env::get_monocore_home_path();
    let db_path = monocore_home_path.join(OCI_DB_FILENAME);
    let layers_dir = monocore_home_path.join(LAYERS_SUBDIR);

    // Get or create a connection pool to the database
    let pool = db::get_or_create_db_pool(&db_path, &db::OCI_DB_MIGRATOR).await?;

    // Get the layers for the image
    let layers = db::get_image_layers(&pool, &image.to_string()).await?;
    tracing::info!("found {} layers for image {}", layers.len(), image);

    // Get the extracted layer paths
    let mut layer_paths = Vec::new();
    for (digest, _, _) in &layers {
        let layer_path = layers_dir.join(format!("{}.{}", digest, EXTRACTED_LAYER_SUFFIX));
        if !layer_path.exists() {
            return Err(MonocoreError::PathNotFound(format!(
                "extracted layer {} not found at {}",
                digest,
                layer_path.display()
            )));
        }
        tracing::info!("found extracted layer: {}", layer_path.display());
        layer_paths.push(layer_path);
    }

    // Get sandbox namespace
    let namespace = menv::create_menv_namespace(config_path, sandbox_config.get_name());

    // Create the scripts directory
    let patch_dir = menv_path.join(PATCH_SUBDIR).join(&namespace);
    let script_dir = patch_dir.join(SANDBOX_SCRIPT_DIR);
    fs::create_dir_all(&script_dir).await?;
    tracing::info!("script_dir: {}", script_dir.display());

    // Clear the patch directory and add the scripts
    let scripts = sandbox_config.get_full_scripts();
    rootfs::patch_rootfs_with_sandbox_scripts(&script_dir, scripts, sandbox_config.get_shell())?;

    // Create the top root path
    let top_root_path = menv_path.join(ROOTFS_SUBDIR).join(&namespace);
    fs::create_dir_all(&top_root_path).await?;
    tracing::info!("top_root_path: {}", top_root_path.display());

    // Add the scripts and rootfs directories to the layer paths
    layer_paths.push(patch_dir);
    layer_paths.push(top_root_path);

    Ok(Rootfs::Overlayfs(layer_paths))
}

async fn setup_native_rootfs(root_path: &Path, sandbox_config: &Sandbox) -> MonocoreResult<Rootfs> {
    // Create the scripts directory
    let scripts_dir = root_path.join(SANDBOX_SCRIPT_DIR);
    fs::create_dir_all(&scripts_dir).await?;

    // Clear the scripts directory and add the scripts
    let scripts = sandbox_config.get_full_scripts();
    rootfs::patch_rootfs_with_sandbox_scripts(&scripts_dir, scripts, sandbox_config.get_shell())?;

    Ok(Rootfs::Native(root_path.to_path_buf()))
}

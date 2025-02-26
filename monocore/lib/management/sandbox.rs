//! Sandbox management functionality for Monocore.
//!
//! This module provides functionality for managing sandboxes, which are isolated execution
//! environments for running applications. It handles sandbox creation, configuration,
//! and execution based on the Monocore configuration file.

use std::path::{Path, PathBuf};

use tokio::{fs, process::Command};
use typed_path::Utf8UnixPathBuf;
use virtualfs::{DEFAULT_NFS_HOST, DEFAULT_NFS_PORT};

use crate::{
    config::{MonocoreConfig, ReferencePath, Sandbox, DEFAULT_MCRUN_EXE_PATH, DEFAULT_WORKDIR},
    management::{db, find, image, rootfs},
    oci::Reference,
    utils::{
        env, DEFAULT_SUBDIR, EXTRACTED_LAYER_SUFFIX, LAYERS_SUBDIR, LOG_SUBDIR, MCRUN_EXE_ENV_VAR,
        MONOCORE_CONFIG_FILENAME, MONOCORE_ENV_DIR, OCI_DB_FILENAME, ROOTFS_SUBDIR,
        SANDBOX_DB_FILENAME, SANDBOX_SCRIPT_DIR, SCRIPTS_SUBDIR,
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
    let full_config_path = project_dir.join(
        config_path
            .clone()
            .unwrap_or_else(|| MONOCORE_CONFIG_FILENAME.into()),
    );

    // Check if config file exists
    if !full_config_path.exists() {
        return Err(MonocoreError::MonocoreConfigNotFound(
            full_config_path.to_string_lossy().to_string(),
        ));
    }

    // Read and parse the monocore.yaml config file
    let config_contents = fs::read_to_string(&full_config_path).await?;
    let config: MonocoreConfig = serde_yaml::from_str(&config_contents)?;

    // Get the sandbox config
    let Some(sandbox_config) = config.get_sandbox(name) else {
        return Err(MonocoreError::SandboxNotFoundInConfig(
            name.to_string(),
            full_config_path,
        ));
    };

    // Get the .menv directory path
    let menv_path = project_dir.join(MONOCORE_ENV_DIR);
    fs::create_dir_all(&menv_path).await?;

    let (root_path, overlayfs) = match sandbox_config.get_image() {
        ReferencePath::Path(root_path) => {
            // Create the scripts directory
            let scripts_dir = root_path.join(SANDBOX_SCRIPT_DIR);
            fs::create_dir_all(&scripts_dir).await?;

            // Clear the scripts directory and add the scripts
            let scripts = sandbox_config.get_full_scripts();
            rootfs::clear_and_add_scripts_to_dir(
                &scripts_dir,
                scripts,
                sandbox_config.get_shell(),
            )?;

            // If relative, join with project_dir, otherwise use as is
            let root_path = if root_path.is_relative() {
                project_dir.join(root_path)
            } else {
                root_path.into()
            };

            (root_path, None)
        }
        ReferencePath::Reference(reference) => {
            let (root_path, layer_paths) = prepare_image_layers(
                reference,
                name,
                sandbox_config,
                &menv_path,
                config_path.as_ref(),
            )
            .await?;

            let port = find::find_available_port(DEFAULT_NFS_HOST, DEFAULT_NFS_PORT).await?;
            tracing::info!("next available port: {}", port);

            (root_path, Some((layer_paths, port)))
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
    tracing::info!("root_path: {}", root_path.display());
    tracing::info!("workdir_path: {}", workdir_path);
    tracing::info!("exec_path: {}", exec_path);

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
        .arg("--root-path")
        .arg(&root_path)
        .arg("--num-vcpus")
        .arg(sandbox_config.get_cpus().to_string())
        .arg("--ram-mib")
        .arg(sandbox_config.get_ram().to_string())
        .arg("--workdir-path")
        .arg(&workdir_path)
        .arg("--exec-path")
        .arg(&exec_path)
        .arg("--forward-output");

    // Add overlayfs arguments if present
    if let Some((layer_paths, port)) = overlayfs {
        command.arg("--overlayfs-layer-paths");
        command.arg(
            layer_paths
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join(","),
        );
        command.arg("--nfs-port");
        command.arg(port.to_string());
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
        tracing::error!("supervisor process exited with status: {}", status);
        return Err(MonocoreError::SupervisorError(format!(
            "supervisor process failed with exit status: {}",
            status
        )));
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

async fn prepare_image_layers(
    image: &Reference,
    name: &str,
    sandbox_config: &Sandbox,
    menv_path: &Path,
    config_path: Option<&PathBuf>,
) -> MonocoreResult<(PathBuf, Vec<PathBuf>)> {
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

    // Get the scripts directory
    let mut scripts_dir = menv_path.join(SCRIPTS_SUBDIR);
    scripts_dir = if let Some(config_path) = config_path {
        scripts_dir.join(config_path.file_name().unwrap())
    } else {
        scripts_dir.join(DEFAULT_SUBDIR)
    };
    scripts_dir = scripts_dir.join(name);
    tracing::info!("scripts_dir: {}", scripts_dir.display());

    // Clear the scripts directory and add the scripts
    let scripts = sandbox_config.get_full_scripts();
    rootfs::clear_and_add_scripts_to_dir(&scripts_dir, scripts, sandbox_config.get_shell())?;

    // Get the rootfs directory
    let mut root_path = menv_path.join(ROOTFS_SUBDIR);
    root_path = if let Some(config_path) = config_path {
        root_path.join(config_path.file_name().unwrap())
    } else {
        root_path.join(DEFAULT_SUBDIR)
    };
    root_path = root_path.join(name);
    tracing::info!("root_path: {}", root_path.display());

    // Add the scripts and rootfs directories to the layer paths
    layer_paths.push(scripts_dir);
    layer_paths.push(root_path.clone());

    Ok((root_path, layer_paths))
}

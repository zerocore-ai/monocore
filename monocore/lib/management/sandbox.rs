//! Sandbox management functionality for Monocore.
//!
//! This module provides functionality for managing sandboxes, which are isolated execution
//! environments for running applications. It handles sandbox creation, configuration,
//! and execution based on the Monocore configuration file.

use std::path::{Path, PathBuf};

use tempfile;
use tokio::{fs, process::Command};
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{
        EnvPair, Monocore, PathPair, PortPair, ReferenceOrPath, Sandbox, DEFAULT_MCRUN_EXE_PATH,
        DEFAULT_MONOCORE_CONFIG_FILENAME,
    },
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
// Constants
//--------------------------------------------------------------------------------------------------

const TEMPORARY_SANDBOX_NAME: &str = "tmp";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Runs a sandbox with the specified configuration and script.
///
/// This function executes a sandbox environment based on the configuration specified in the Monocore
/// config file. It handles both native rootfs and image-based rootfs setups.
///
/// ## Arguments
///
/// * `sandbox` - The name of the sandbox to run as defined in the Monocore config file
/// * `script` - The name of the script to execute within the sandbox (e.g., "start", "shell")
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_path` - Optional path to the Monocore config file. If None, uses default filename
/// * `args` - Additional arguments to pass to the sandbox script
///
/// ## Returns
///
/// Returns `Ok(())` if the sandbox runs and exits successfully, or a `MonocoreError` if:
/// - The config file is not found
/// - The specified sandbox is not found in the config
/// - The supervisor process fails to start or exits with an error
/// - Any filesystem operations fail
///
/// ## Example
///
/// ```no_run
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Run a sandbox named "dev" with the "start" script
///     sandbox::run_sandbox(
///         "dev",
///         "start",
///         None,
///         None,
///         vec![]
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn run_sandbox(
    sandbox: &str,
    script: &str,
    project_dir: Option<PathBuf>,
    config_path: Option<PathBuf>,
    args: Vec<String>,
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
    let config: Monocore = serde_yaml::from_str(&config_contents)?;

    tracing::debug!("config: {:?}", config);

    // Get the sandbox config
    let Some(sandbox_config) = config.get_sandbox(sandbox) else {
        return Err(MonocoreError::SandboxNotFoundInConfig(
            sandbox.to_string(),
            full_config_path,
        ));
    };

    tracing::debug!("sandbox_config: {:?}", sandbox_config);

    // Ensure the .menv files exist
    menv::ensure_menv_files(&canonical_project_dir).await?;
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);

    let rootfs = match sandbox_config.get_image() {
        ReferenceOrPath::Path(root_path) => {
            setup_native_rootfs(&canonical_project_dir.join(root_path), sandbox_config).await?
        }
        ReferenceOrPath::Reference(reference) => {
            setup_image_rootfs(reference, sandbox_config, &menv_path, config_path.as_ref()).await?
        }
    };

    // Log directory
    let log_dir = menv_path.join(LOG_SUBDIR);
    fs::create_dir_all(&log_dir).await?;

    // Sandbox database path
    let sandbox_db_path = menv_path.join(SANDBOX_DB_FILENAME);

    // Get the exec path
    let exec_path = format!("/{}/{}", SANDBOX_SCRIPT_DIR, script);

    tracing::info!("starting sandbox supervisor...");

    tracing::debug!("rootfs: {:?}", rootfs);
    tracing::debug!("exec_path: {}", exec_path);

    let mcrun_path =
        monoutils::path::resolve_env_path(MCRUN_EXE_ENV_VAR, &*DEFAULT_MCRUN_EXE_PATH)?;

    let mut command = Command::new(mcrun_path);
    command
        .arg("supervisor")
        .arg("--log-dir")
        .arg(&log_dir)
        .arg("--child-name")
        .arg(sandbox)
        .arg("--sandbox-db-path")
        .arg(&sandbox_db_path)
        .arg("--exec-path")
        .arg(&exec_path)
        .arg("--forward-output");

    if let Some(cpus) = sandbox_config.get_cpus() {
        command.arg("--num-vcpus").arg(cpus.to_string());
    }

    if let Some(ram) = sandbox_config.get_ram() {
        command.arg("--ram-mib").arg(ram.to_string());
    }

    if let Some(workdir) = sandbox_config.get_workdir() {
        command.arg("--workdir-path").arg(workdir);
    }

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
        tracing::error!(
            "child process — supervisor — exited with status: {}",
            status
        );
        return Err(MonocoreError::SupervisorError(format!(
            "child process — supervisor — failed with exit status: {}",
            status
        )));
    }

    Ok(())
}

/// Creates and runs a temporary sandbox from an OCI image.
///
/// This function creates a temporary sandbox environment from a container image without requiring
/// a Monocore configuration file. It's useful for quick, one-off sandbox executions.
/// The temporary sandbox and its associated files are automatically cleaned up after execution.
///
/// # Arguments
///
/// * `image` - The OCI image reference to use as the base for the sandbox
/// * `script` - The name of the script to execute within the sandbox
/// * `cpus` - Optional number of virtual CPUs to allocate to the sandbox
/// * `ram` - Optional amount of RAM in MiB to allocate to the sandbox
/// * `volumes` - List of volume mappings in the format "host_path:guest_path"
/// * `ports` - List of port mappings in the format "host_port:guest_port"
/// * `envs` - List of environment variables in the format "KEY=VALUE"
/// * `workdir` - Optional working directory path inside the sandbox
///
/// # Returns
///
/// Returns `Ok(())` if the temporary sandbox runs and exits successfully, or a `MonocoreError` if:
/// - The image cannot be pulled or found
/// - The sandbox configuration is invalid
/// - The supervisor process fails to start or exits with an error
/// - Any filesystem operations fail
///
/// # Example
///
/// ```no_run
/// use monocore::oci::Reference;
/// use typed_path::Utf8UnixPathBuf;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let image = "ubuntu:latest".parse::<Reference>()?;
///
///     // Run a temporary Ubuntu sandbox with custom resources
///     sandbox::run_temp_sandbox(
///         &image,
///         "start",
///         Some(2),           // 2 CPUs
///         Some(1024),        // 1GB RAM
///         vec![              // Mount host's /tmp to sandbox's /data
///             "/tmp:/data".to_string()
///         ],
///         vec![              // Map host port 8080 to sandbox port 80
///             "8080:80".to_string()
///         ],
///         vec![              // Set environment variables
///             "DEBUG=1".to_string()
///         ],
///         Some("/app".into()) // Set working directory
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn run_temp_sandbox(
    image: &Reference,
    script: &str,
    cpus: Option<u8>,
    ram: Option<u32>,
    volumes: Vec<String>,
    ports: Vec<String>,
    envs: Vec<String>,
    workdir: Option<Utf8UnixPathBuf>,
) -> MonocoreResult<()> {
    // Create a temporary directory without losing the TempDir guard for automatic cleanup
    let temp_dir = tempfile::tempdir()?;
    let temp_dir_path = temp_dir.path().to_path_buf();

    // Initialize menv in the temporary directory
    menv::init_menv(Some(temp_dir_path.clone())).await?;

    // Parse the volume, port, and env strings into their respective types
    let volumes: Vec<PathPair> = volumes.into_iter().filter_map(|v| v.parse().ok()).collect();
    let ports: Vec<PortPair> = ports.into_iter().filter_map(|p| p.parse().ok()).collect();
    let envs: Vec<EnvPair> = envs.into_iter().filter_map(|e| e.parse().ok()).collect();

    // Build the temporary sandbox configuration.
    let sandbox = {
        let mut b = Sandbox::builder()
            .name(TEMPORARY_SANDBOX_NAME)
            .image(ReferenceOrPath::Reference(image.clone()));

        if let Some(cpus) = cpus {
            b = b.cpus(cpus);
        }

        if let Some(ram) = ram {
            b = b.ram(ram);
        }

        if let Some(workdir) = workdir {
            b = b.workdir(workdir);
        }

        if !volumes.is_empty() {
            b = b.volumes(volumes);
        }

        if !ports.is_empty() {
            b = b.ports(ports);
        }

        if !envs.is_empty() {
            b = b.envs(envs);
        }

        b.build()
    };

    // Create the monocore config with the temporary sandbox
    let config = Monocore::builder()
        .sandboxes(vec![sandbox])
        .build_unchecked();

    // Write the config to the temporary directory
    let config_path = temp_dir_path.join(DEFAULT_MONOCORE_CONFIG_FILENAME);
    tokio::fs::write(&config_path, serde_yaml::to_string(&config)?).await?;

    // Run the sandbox with the temporary configuration
    run_sandbox(
        TEMPORARY_SANDBOX_NAME,
        script,
        Some(temp_dir_path.clone()),
        None,
        vec![],
    )
    .await?;

    tracing::info!("temporary sandbox exited successfully");

    // Explicitly close the TempDir to clean up the temporary directory
    temp_dir.close()?;

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

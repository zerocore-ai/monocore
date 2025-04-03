//! Sandbox management functionality for Monocore.
//!
//! This module provides functionality for managing sandboxes, which are isolated execution
//! environments for running applications. It handles sandbox creation, configuration,
//! and execution based on the Monocore configuration file.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};
use tempfile;
use tokio::{fs, process::Command};
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{
        EnvPair, Monocore, PathPair, PortPair, ReferenceOrPath, Sandbox, DEFAULT_MCRUN_EXE_PATH,
        DEFAULT_SCRIPT,
    },
    management::{config, db, image, menv, rootfs},
    oci::Reference,
    utils::{
        env, EXTRACTED_LAYER_SUFFIX, LAYERS_SUBDIR, LOG_SUBDIR, MCRUN_EXE_ENV_VAR,
        MONOCORE_CONFIG_FILENAME, MONOCORE_ENV_DIR, OCI_DB_FILENAME, PATCH_SUBDIR, RW_SUBDIR,
        SANDBOX_DB_FILENAME, SANDBOX_SCRIPT_DIR, SHELL_SCRIPT_NAME,
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
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
/// * `args` - Additional arguments to pass to the sandbox script
/// * `detach` - Whether to run the sandbox in the background
/// * `exec` - Optional command to execute within the sandbox. Overrides `script` if provided.
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
/// use monocore::management::sandbox;
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
pub async fn run(
    sandbox_name: &str,
    script_name: Option<&str>,
    project_dir: Option<&Path>,
    config_file: Option<&str>,
    args: Vec<String>,
    detach: bool,
    exec: Option<&str>,
) -> MonocoreResult<()> {
    let script_name = match script_name {
        Some(script_name) => script_name,
        None => DEFAULT_SCRIPT,
    };

    // Load the configuration
    let (config, canonical_project_dir, config_file) =
        config::load_config(project_dir, config_file).await?;

    // Ensure the .menv files exist
    let menv_path = canonical_project_dir.join(MONOCORE_ENV_DIR);
    menv::ensure_menv_files(&menv_path).await?;

    // Get the sandbox config
    let Some(sandbox_config) = config.get_sandbox(sandbox_name) else {
        return Err(MonocoreError::SandboxNotFoundInConfig(
            sandbox_name.to_string(),
            canonical_project_dir.join(&config_file),
        ));
    };

    tracing::debug!("original sandbox_config: {:#?}", sandbox_config);

    // Sandbox database path
    let sandbox_db_path = menv_path.join(SANDBOX_DB_FILENAME);

    // Get sandbox database connection pool
    let sandbox_pool = db::get_or_create_pool(&sandbox_db_path, &db::SANDBOX_DB_MIGRATOR).await?;

    // Get the config last modified timestamp
    let config_last_modified: DateTime<Utc> = fs::metadata(&config_file).await?.modified()?.into();

    let rootfs = match sandbox_config.get_image() {
        ReferenceOrPath::Path(root_path) => {
            setup_native_rootfs(
                &canonical_project_dir.join(root_path),
                sandbox_name,
                sandbox_config,
                &config_file,
                &config_last_modified,
                &sandbox_pool,
                script_name,
            )
            .await?
        }
        ReferenceOrPath::Reference(reference) => {
            setup_image_rootfs(
                reference,
                sandbox_name,
                sandbox_config,
                &menv_path,
                &config_file,
                &config_last_modified,
                &sandbox_pool,
                script_name,
            )
            .await?
        }
    };

    // Log directory
    let log_dir = menv_path.join(LOG_SUBDIR);
    fs::create_dir_all(&log_dir).await?;

    // Get the exec path. If exec is provided, use it as the exec path.
    // Otherwise, use the script name.
    let exec_path = match exec {
        Some(exec) => exec.to_string(),
        None => format!("/{}/{}", SANDBOX_SCRIPT_DIR, script_name),
    };

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
        .arg("--sandbox-name")
        .arg(sandbox_name)
        .arg("--config-file")
        .arg(&config_file)
        .arg("--config-last-modified")
        .arg(&config_last_modified.to_rfc3339())
        .arg("--sandbox-db-path")
        .arg(&sandbox_db_path)
        .arg("--scope")
        .arg(sandbox_config.get_scope().to_string())
        .arg("--exec-path")
        .arg(&exec_path);

    // CPU
    if let Some(cpus) = sandbox_config.get_cpus() {
        command.arg("--num-vcpus").arg(cpus.to_string());
    }

    // RAM
    if let Some(ram) = sandbox_config.get_ram() {
        command.arg("--ram-mib").arg(ram.to_string());
    }

    // Workdir
    if let Some(workdir) = sandbox_config.get_workdir() {
        command.arg("--workdir-path").arg(workdir);
    }

    // Env
    for env in sandbox_config.get_envs() {
        command.arg("--env").arg(env.to_string());
    }

    // Ports
    for port in sandbox_config.get_ports() {
        command.arg("--port").arg(port.to_string());
    }

    // Volumes
    for volume in sandbox_config.get_volumes() {
        command.arg("--mapped-dir").arg(volume.to_string());
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

    // In detached mode, ignore the i/o of the supervisor process.
    if detach {
        // Safety:
        // We call `libc::setsid()` to detach the child process from the parent's session and controlling terminal.
        //
        // This call is safe in our context because:
        // - It only creates a new session and process group for the child, which is exactly what we intend.
        // - We are not modifying any shared mutable state.
        // - The call has no side-effects beyond detaching the process.
        //
        // ASCII diagram illustrating the detachment:
        //
        //      [ Main Process ]
        //             │
        //             ├── spawns ──► [ Supervisor ]
        //                                 │
        //                                 └─ calls setsid() ─► [ New Session & Process Group ]
        //                                               (Detached)
        //
        // This ensures that the supervisor runs independently, even if the orchestrator exits.
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        // TODO: Redirect to log file
        // Redirect the i/o to /dev/null
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.stdin(Stdio::null());
    } else {
        command.arg("--forward-output");
    }

    let mut child = command.spawn()?;

    tracing::info!(
        "started supervisor process with PID: {}",
        child.id().unwrap_or(0)
    );

    // If in detached mode, don't wait for the child process to complete
    if detach {
        return Ok(());
    }

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
/// use monocore::management::sandbox;
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
pub async fn run_temp(
    image: &Reference,
    script: Option<&str>,
    cpus: Option<u8>,
    ram: Option<u32>,
    volumes: Vec<String>,
    ports: Vec<String>,
    envs: Vec<String>,
    workdir: Option<Utf8UnixPathBuf>,
    exec: Option<&str>,
    args: Vec<String>,
) -> MonocoreResult<()> {
    // Create a temporary directory without losing the TempDir guard for automatic cleanup
    let temp_dir = tempfile::tempdir()?;
    let temp_dir_path = temp_dir.path().to_path_buf();

    // Initialize menv in the temporary directory
    menv::initialize(Some(temp_dir_path.clone())).await?;

    // Parse the volume, port, and env strings into their respective types
    let volumes: Vec<PathPair> = volumes.into_iter().filter_map(|v| v.parse().ok()).collect();
    let ports: Vec<PortPair> = ports.into_iter().filter_map(|p| p.parse().ok()).collect();
    let envs: Vec<EnvPair> = envs.into_iter().filter_map(|e| e.parse().ok()).collect();

    // Build the temporary sandbox configuration.
    let sandbox = {
        let mut b = Sandbox::builder().image(ReferenceOrPath::Reference(image.clone()));

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
        .sandboxes([(TEMPORARY_SANDBOX_NAME.to_string(), sandbox)])
        .build_unchecked();

    // Write the config to the temporary directory
    let config_path = temp_dir_path.join(MONOCORE_CONFIG_FILENAME);
    tokio::fs::write(&config_path, serde_yaml::to_string(&config)?).await?;

    // Run the sandbox with the temporary configuration
    run(
        TEMPORARY_SANDBOX_NAME,
        script,
        Some(&temp_dir_path),
        None,
        args,
        false,
        exec,
    )
    .await?;

    // Explicitly close the TempDir to clean up the temporary directory
    temp_dir.close()?;
    tracing::info!("temporary sandbox directory cleaned up");

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

async fn setup_image_rootfs(
    image: &Reference,
    sandbox_name: &str,
    sandbox_config: &Sandbox,
    menv_path: &Path,
    config_file: &str,
    config_last_modified: &DateTime<Utc>,
    sandbox_pool: &Pool<Sqlite>,
    script_name: &str,
) -> MonocoreResult<Rootfs> {
    // Pull the image from the registry
    tracing::info!("pulling image: {}", image);
    image::pull(image.clone(), true, false, None).await?;

    // Get the monocore home path and database path
    let monocore_home_path = env::get_monocore_home_path();
    let db_path = monocore_home_path.join(OCI_DB_FILENAME);
    let layers_dir = monocore_home_path.join(LAYERS_SUBDIR);

    // Get or create a connection pool to the database
    let pool = db::get_or_create_pool(&db_path, &db::OCI_DB_MIGRATOR).await?;

    // Get the layers for the image
    let layers = db::get_image_layers(&pool, &image.to_string()).await?;
    tracing::info!("found {} layers for image {}", layers.len(), image);

    // Get the extracted layer paths
    let mut layer_paths = Vec::new();
    for layer in &layers {
        let layer_path = layers_dir.join(format!("{}.{}", layer.digest, EXTRACTED_LAYER_SUFFIX));
        if !layer_path.exists() {
            return Err(MonocoreError::PathNotFound(format!(
                "extracted layer {} not found at {}",
                layer.digest,
                layer_path.display()
            )));
        }
        tracing::info!("found extracted layer: {}", layer_path.display());
        layer_paths.push(layer_path);
    }

    // Get sandbox namespace
    let namespaced_name = PathBuf::from(config_file).join(sandbox_name);

    // Create the scripts directory
    let patch_dir = menv_path.join(PATCH_SUBDIR).join(&namespaced_name);
    let script_dir = patch_dir.join(SANDBOX_SCRIPT_DIR);
    fs::create_dir_all(&script_dir).await?;
    tracing::info!("script_dir: {}", script_dir.display());

    // Validate script exists
    let scripts = sandbox_config.get_full_scripts();
    if script_name != SHELL_SCRIPT_NAME && !scripts.contains_key(script_name) {
        return Err(MonocoreError::ScriptNotFoundInSandbox(
            script_name.to_string(),
            sandbox_name.to_string(),
        ));
    }

    // Create the top root path
    let top_rw_path = menv_path.join(RW_SUBDIR).join(&namespaced_name);
    fs::create_dir_all(&top_rw_path).await?;
    tracing::info!("top_rw_path: {}", top_rw_path.display());

    // Check if we need to patch rootfs (scripts, volumes, etc.)
    let should_patch = has_sandbox_config_changed(
        sandbox_pool,
        sandbox_name,
        config_file,
        config_last_modified,
    )
    .await?;

    // Only patch if sandbox doesn't exist or config has changed
    if should_patch {
        tracing::info!("patching sandbox - config has changed");

        // If `/.sandbox_scripts` exists at the top layer, delete it
        let rw_scripts_dir = top_rw_path.join(SANDBOX_SCRIPT_DIR);
        if rw_scripts_dir.exists() {
            fs::remove_dir_all(&rw_scripts_dir).await?;
        }

        // Patch with sandbox scripts
        rootfs::patch_with_sandbox_scripts(&script_dir, scripts, sandbox_config.get_shell())
            .await?;

        // Patch with volume mounts if there are any volumes defined
        let volumes = &sandbox_config.get_volumes();
        if !volumes.is_empty() {
            tracing::info!("patching with {} volume mounts", volumes.len());
            rootfs::patch_with_virtiofs_mounts(&patch_dir, volumes).await?;
        }
    } else {
        tracing::info!("skipping sandbox patch - config unchanged");
    }

    // Add the scripts and rootfs directories to the layer paths
    layer_paths.push(patch_dir);
    layer_paths.push(top_rw_path);

    Ok(Rootfs::Overlayfs(layer_paths))
}

async fn setup_native_rootfs(
    root_path: &Path,
    sandbox_name: &str,
    sandbox_config: &Sandbox,
    config_file: &str,
    config_last_modified: &DateTime<Utc>,
    sandbox_pool: &Pool<Sqlite>,
    script_name: &str,
) -> MonocoreResult<Rootfs> {
    // Create the scripts directory
    let scripts_dir = root_path.join(SANDBOX_SCRIPT_DIR);
    fs::create_dir_all(&scripts_dir).await?;

    // Validate script exists
    let scripts = sandbox_config.get_full_scripts();
    if script_name != SHELL_SCRIPT_NAME && !scripts.contains_key(script_name) {
        return Err(MonocoreError::ScriptNotFoundInSandbox(
            script_name.to_string(),
            sandbox_name.to_string(),
        ));
    }

    // Check if we need to patch rootfs (scripts, volumes, etc.)
    let should_patch = has_sandbox_config_changed(
        sandbox_pool,
        sandbox_name,
        config_file,
        config_last_modified,
    )
    .await?;

    // Only patch if sandbox doesn't exist or config has changed
    if should_patch {
        tracing::info!("patching sandbox - config has changed");

        // Patch with sandbox scripts
        rootfs::patch_with_sandbox_scripts(&scripts_dir, scripts, sandbox_config.get_shell())
            .await?;

        // Patch with volume mounts if there are any volumes defined
        let volumes = &sandbox_config.get_volumes();
        if !volumes.is_empty() {
            tracing::info!("patching with {} volume mounts", volumes.len());
            // For native rootfs, mount points should be created under the root path
            rootfs::patch_with_virtiofs_mounts(root_path, volumes).await?;
        }
    } else {
        tracing::info!("skipping sandbox patch - config unchanged");
    }

    Ok(Rootfs::Native(root_path.to_path_buf()))
}

/// Checks if a sandbox's configuration has changed by comparing the current config's last modified
/// timestamp with the stored timestamp in the database. Returns true if the sandbox doesn't exist
/// or if the config has been modified since the last run.
async fn has_sandbox_config_changed(
    sandbox_pool: &Pool<Sqlite>,
    sandbox_name: &str,
    config_file: &str,
    config_last_modified: &DateTime<Utc>,
) -> MonocoreResult<bool> {
    // Check if sandbox exists and config hasn't changed
    let sandbox = db::get_sandbox(sandbox_pool, sandbox_name, config_file).await?;
    Ok(match sandbox {
        Some(sandbox) => {
            // Compare timestamps to see if config has changed
            sandbox.config_last_modified != *config_last_modified
        }
        None => true, // No existing sandbox, need to patch
    })
}

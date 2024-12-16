use std::{
    env,
    net::Ipv4Addr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{self, Stdio},
    sync::Arc,
    time::Duration,
};

use oci_spec::image::ImageConfiguration;
use serde_json::from_str;
use sysinfo::System;
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{broadcast, Mutex},
    task::JoinHandle,
    time,
    time::interval,
};
use tracing::{error, info, warn};

use crate::{
    config::{Group, Service},
    runtime::MicroVmStatus,
    utils::{self, LOG_SUBDIR, OCI_CONFIG_FILENAME, OCI_REPO_SUBDIR, OCI_SUBDIR, STATE_SUBDIR},
    MonocoreError, MonocoreResult,
};

use super::{log::RotatingLog, MicroVmState};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The supervisor of the micro VMs.
#[derive(Debug)]
pub struct Supervisor {
    /// The state of the micro VM process.
    state: Arc<Mutex<MicroVmState>>,

    /// The home directory of monocore.
    home_dir: PathBuf,

    /// The path to the state file of the micro VM process.
    runtime_state_path: PathBuf,

    /// The path to the stdout log file of the micro VM process.
    stdout_log_path: PathBuf,

    /// The path to the stderr log file of the micro VM process.
    stderr_log_path: PathBuf,

    /// The channel to send shutdown signals to the micro VM process.
    shutdown_tx: broadcast::Sender<()>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Supervisor {
    /// Creates a new Supervisor instance.
    pub async fn new(
        home_dir: impl AsRef<Path>,
        service: Service,
        group: Group,
        group_ip: Option<Ipv4Addr>,
        rootfs_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        let home_dir = home_dir.as_ref().to_path_buf();
        let state_dir = home_dir.join(STATE_SUBDIR);
        let log_dir = home_dir.join(LOG_SUBDIR);

        // Generate unique IDs for the files
        let service_name = service.get_name();

        // Create paths with service name for better identification
        let runtime_state_path =
            state_dir.join(format!("{}__{}.json", service_name, process::id()));
        let stdout_log_path = log_dir.join(format!("{}.stdout.log", service_name));
        let stderr_log_path = log_dir.join(format!("{}.stderr.log", service_name));

        // Create directories with proper permissions
        for dir in [&state_dir, &log_dir] {
            fs::create_dir_all(dir).await?;
            #[cfg(unix)]
            {
                let metadata = fs::metadata(dir).await?;
                let mut perms = metadata.permissions();
                perms.set_mode(0o755); // rwxr-xr-x
                fs::set_permissions(dir, perms).await?;
            }
        }

        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            state: Arc::new(Mutex::new(MicroVmState::new(
                service,
                group,
                group_ip,
                rootfs_path,
            ))),
            home_dir,
            runtime_state_path,
            stdout_log_path,
            stderr_log_path,
            shutdown_tx,
        })
    }

    /// Starts the supervised micro VM process.
    ///
    /// This method:
    /// 1. Spawns the micro VM subprocess
    /// 2. Sets up stdout/stderr logging
    /// 3. Initializes process monitoring
    pub async fn start(&mut self) -> MonocoreResult<JoinHandle<MonocoreResult<()>>> {
        {
            let mut state = self.state.lock().await;
            state.set_status(MicroVmStatus::Starting);
            state.save(&self.runtime_state_path).await?;
        }

        let current_exe = env::current_exe()?;

        // Get all the needed data under a single lock
        let (service_json, group_json, local_only_json, group_ip_json, rootfs_path) = {
            let mut state = self.state.lock().await;
            let service = state.get_service_mut();

            // Update service with OCI config defaults
            self.update_service_with_oci_config(service).await?;

            let service_json = serde_json::to_string(service)?;
            let group_json = serde_json::to_string(state.get_group())?;

            let local_only_json = serde_json::to_string(state.get_group().get_local_only())?;
            let group_ip_json =
                serde_json::to_string(&state.get_group_ip().unwrap_or(Ipv4Addr::LOCALHOST))?;
            let rootfs_path = state.get_rootfs_path().to_str().unwrap().to_string();
            (
                service_json,
                group_json,
                local_only_json,
                group_ip_json,
                rootfs_path,
            )
        };

        // Start the micro VM sub process
        let mut child = Command::new(current_exe)
            .args([
                "--run-microvm",
                &service_json,
                &group_json,
                &group_ip_json,
                &rootfs_path,
                &local_only_json,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Set the status and PID of the micro VM process
        {
            let mut state = self.state.lock().await;
            state.set_status(MicroVmStatus::Started);
            state.set_pid(child.id());
            state.save(&self.runtime_state_path).await?;
        }

        // Handle stdout
        let stdout = child.stdout.take().unwrap();
        let stdout_path = self.stdout_log_path.clone();
        let service_name = self.state.lock().await.get_service().get_name().to_string();
        let stdout_handle = tokio::spawn(async move {
            println!("Starting stdout log rotation for {}", service_name); // TODO: Remove
            let mut rotating_log = match RotatingLog::new(&stdout_path, None).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to create stdout rotating log: {}", e);
                    return;
                }
            };

            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Format the log entry with timestamp in standard log format
                let now = chrono::Utc::now();
                let formatted_line = format!(
                    "{} INFO [{}] {}\n",
                    now.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                    service_name,
                    line
                );

                if let Err(e) = rotating_log.write_all(formatted_line.as_bytes()).await {
                    error!("Failed to write to stdout log: {}", e);
                }

                if let Err(e) = rotating_log.flush().await {
                    error!("Failed to flush stdout log: {}", e);
                }
            }
        });

        // Handle stderr
        let stderr = child.stderr.take().unwrap();
        let stderr_path = self.stderr_log_path.clone();
        let service_name = self.state.lock().await.get_service().get_name().to_string();
        let stderr_handle = tokio::spawn(async move {
            println!("Starting stderr log rotation for {}", service_name); // TODO: Remove
            let mut rotating_log = match RotatingLog::new(&stderr_path, None).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to create stderr rotating log: {}", e);
                    return;
                }
            };

            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Format the log entry with timestamp in standard log format
                let now = chrono::Utc::now();
                let formatted_line = format!(
                    "{} ERROR [{}] {}\n",
                    now.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                    service_name,
                    line
                );

                if let Err(e) = rotating_log.write_all(formatted_line.as_bytes()).await {
                    error!("Failed to write to stderr log: {}", e);
                }

                if let Err(e) = rotating_log.flush().await {
                    error!("Failed to flush stderr log: {}", e);
                }
            }
        });

        // Handle metrics
        let metrics_state = self.state.clone();
        let metrics_runtime_state_path = self.runtime_state_path.clone();
        let pid = child.id().unwrap();
        let metrics_handle = tokio::spawn(async move {
            let mut sys = System::new();

            // Initial refresh to start CPU measurement
            sys.refresh_all();
            let mut interval = interval(Duration::from_secs(2));

            loop {
                interval.tick().await;
                sys.refresh_all();

                if let Some(process) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    // Get CPU usage using ps command because sysinfo is not accurate
                    let output = match Command::new("ps")
                        .args(["-p", &pid.to_string(), "-o", "%cpu="])
                        .output()
                        .await
                    {
                        Ok(output) => output,
                        Err(e) => {
                            error!("Failed to execute ps command: {}", e);
                            continue;
                        }
                    };

                    let cpu_usage = match String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .parse::<f32>()
                    {
                        Ok(usage) => usage,
                        Err(e) => {
                            error!("Failed to parse CPU usage: {}", e);
                            continue;
                        }
                    };

                    let memory_usage = process.memory();
                    let disk_usage = process.disk_usage();

                    // Update metrics in state
                    let mut state = metrics_state.lock().await;
                    state.get_metrics_mut().set_cpu_usage(cpu_usage);
                    state.get_metrics_mut().set_memory_usage(memory_usage);
                    state
                        .get_metrics_mut()
                        .set_disk_read_bytes(disk_usage.read_bytes);
                    state
                        .get_metrics_mut()
                        .set_disk_write_bytes(disk_usage.written_bytes);
                    state
                        .get_metrics_mut()
                        .set_total_disk_read_bytes(disk_usage.total_read_bytes);
                    state
                        .get_metrics_mut()
                        .set_total_disk_write_bytes(disk_usage.total_written_bytes);

                    // Save updated state
                    if let Err(e) = state.save(&metrics_runtime_state_path).await {
                        error!("Failed to save state with updated metrics: {}", e);
                    }
                } else {
                    // Process no longer exists
                    break;
                }
            }
        });

        // Create shutdown receiver before spawning the main handle
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let runtime_state_path = self.runtime_state_path.clone();

        // Update the handle spawning to include metrics_handle
        let handle = tokio::spawn(async move {
            let result = tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown signal, terminating micro VM process");
                    let _ = child.kill().await;
                    Ok(())
                }
                status = child.wait() => {
                    match status {
                        Ok(exit_status) => {
                            // Clean up runtime state file if it still exists
                            info!(
                                "Removing runtime state file: {}",
                                runtime_state_path.display()
                            );

                            if let Err(e) = fs::remove_file(&runtime_state_path).await {
                                warn!(
                                    "Failed to remove runtime state file {}: {}",
                                    runtime_state_path.display(),
                                    e
                                );
                            }

                            info!(
                                "Micro VM process exited with status, cleaning up: {}",
                                exit_status
                            );
                            Ok(())
                        }
                        Err(e) => {
                            error!("Error waiting for micro VM process: {}", e);
                            Err(MonocoreError::ProcessWaitError(e.to_string()))
                        }
                    }
                }
            };

            // Ensure log tasks and metrics task are cleaned up
            stdout_handle.abort();
            stderr_handle.abort();
            metrics_handle.abort();

            result
        });

        Ok(handle)
    }

    /// Stops the supervised micro VM sub process.
    ///
    /// Sends a shutdown signal to the process and waits for it to terminate.
    pub async fn stop(&mut self) -> MonocoreResult<()> {
        {
            let mut state = self.state.lock().await;
            state.set_status(MicroVmStatus::Stopping);
            state.save(&self.runtime_state_path).await?;
        }

        if let Err(e) = self.shutdown_tx.send(()) {
            error!("Failed to send shutdown signal: {}", e);
        }

        // Wait a bit for the process to clean up
        time::sleep(time::Duration::from_secs(1)).await;

        // Clean up runtime state file if it still exists
        info!(
            "Removing runtime state file: {}",
            self.runtime_state_path.display()
        );

        if let Err(e) = fs::remove_file(&self.runtime_state_path).await {
            warn!(
                "Failed to remove runtime state file {}: {}",
                self.runtime_state_path.display(),
                e
            );
        }

        {
            let mut state = self.state.lock().await;
            state.set_status(MicroVmStatus::Stopped { exit_code: 0 });
        }
        Ok(())
    }

    /// Saves the current runtime state to disk.
    ///
    /// The state is saved to a JSON file at the path specified by `runtime_state_path`.
    pub async fn save_runtime_state(&self) -> MonocoreResult<()> {
        let state_json = serde_json::to_string_pretty(&*self.state.lock().await)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.runtime_state_path)
            .await?;

        file.write_all(state_json.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    /// Loads the runtime state from disk.
    ///
    /// Reads and deserializes the state from the JSON file at `runtime_state_path`.
    pub async fn load_runtime_state(&self) -> MonocoreResult<MicroVmState> {
        let contents = fs::read_to_string(&self.runtime_state_path).await?;
        let state = serde_json::from_str(&contents)?;
        Ok(state)
    }

    /// Updates service properties with defaults from OCI config.json if they are not specified.
    /// The config.json file is expected to be in <home_dir>/oci/repo/<image_name>/.
    async fn update_service_with_oci_config(&self, service: &mut Service) -> MonocoreResult<()> {
        // Get base image name from service config
        let base_image = match service.get_base() {
            Some(base) => base,
            None => return Ok(()), // No base image, nothing to do
        };

        // Parse image reference to get the repository tag directory name
        let (_, _, repo_tag) = utils::parse_image_ref(base_image)?;

        // Construct path to config.json
        let config_path = self
            .home_dir
            .join(OCI_SUBDIR)
            .join(OCI_REPO_SUBDIR)
            .join(repo_tag)
            .join(OCI_CONFIG_FILENAME);

        // Read and parse config.json
        let config_str = match fs::read_to_string(&config_path).await {
            Ok(content) => content,
            Err(e) => {
                error!(
                    "Failed to read OCI config.json at {}: {}",
                    config_path.display(),
                    e
                );
                return Ok(());
            }
        };

        let config: ImageConfiguration = match from_str(&config_str) {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to parse OCI config.json: {}", e);
                return Ok(());
            }
        };

        // Get the config section which contains the defaults
        let config = match config.config() {
            Some(config) => config,
            None => return Ok(()),
        };

        // Update workdir if not set
        if service.get_workdir().is_none() {
            if let Some(working_dir) = config.working_dir() {
                service.set_workdir(working_dir.to_string());
            }
        }

        // Update command and args if not set
        if service.get_command().is_none() {
            // First try entrypoint + cmd
            if let Some(entrypoint) = config.entrypoint() {
                if !entrypoint.is_empty() {
                    // Use first item as command and rest as args
                    let mut entrypoint = entrypoint.clone();
                    if let Some(command) = entrypoint.first() {
                        service.set_command(command.clone());
                        // Add remaining entrypoint items as args
                        let mut args = entrypoint.split_off(1);
                        // Add cmd as additional args if present
                        if let Some(cmd) = config.cmd() {
                            args.extend(cmd.iter().cloned());
                        }
                        service.set_args(args);
                    }
                }
            } else if let Some(cmd) = config.cmd() {
                if !cmd.is_empty() {
                    // Use first item as command and rest as args
                    let mut cmd = cmd.clone();
                    if let Some(command) = cmd.first() {
                        service.set_command(command.clone());
                        service.set_args(cmd.split_off(1));
                    }
                }
            }
        }

        // Prepend config env to service envs
        if let Some(env) = config.env() {
            // Get existing service envs
            let mut new_envs = env.clone();
            new_envs.extend(service.get_envs().iter().map(|e| e.to_string()));
            service.set_envs(new_envs);
        }

        // NOTE: We intentionally do not use exposed_ports from OCI config.
        // In OCI/Docker, EXPOSE (which becomes exposed_ports in config.json) only documents which ports
        // a container uses internally. It does not define port mappings between host and container.

        Ok(())
    }
}

use std::{
    env,
    net::Ipv4Addr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{self, Stdio},
    sync::Arc,
    time::Duration,
};

use sysinfo::System;
use tokio::{
    fs::{self, File, OpenOptions},
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
    utils::{MONOCORE_LOG_DIR, MONOCORE_STATE_DIR},
    MonocoreError, MonocoreResult,
};

use super::MicroVmState;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The supervisor of the micro VMs.
#[derive(Debug)]
pub struct Supervisor {
    /// The state of the micro VM process.
    state: Arc<Mutex<MicroVmState>>,

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
    const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10MB max log size

    /// Creates a new Supervisor instance.
    pub async fn new(
        service: Service,
        group: Group,
        group_ip: Option<Ipv4Addr>,
        rootfs_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        // Generate unique IDs for the files
        let service_name = service.get_name();

        // Create paths with service name for better identification
        let runtime_state_path =
            MONOCORE_STATE_DIR.join(format!("{}__{}.json", service_name, process::id()));
        let stdout_log_path = MONOCORE_LOG_DIR.join(format!("{}.stdout.log", service_name));
        let stderr_log_path = MONOCORE_LOG_DIR.join(format!("{}.stderr.log", service_name));

        // Create directories with proper permissions
        for dir in [&*MONOCORE_STATE_DIR, &*MONOCORE_LOG_DIR] {
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
            runtime_state_path,
            stdout_log_path,
            stderr_log_path,
            shutdown_tx,
        })
    }

    /// Creates a log file with proper permissions and rotation
    async fn create_log_file(path: &Path) -> MonocoreResult<File> {
        // Create new log file with proper permissions
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path)
            .await?;

        let mut perms = file.metadata().await?.permissions();
        perms.set_mode(0o644); // rw-r--r--
        file.set_permissions(perms).await?;

        Ok(file)
    }

    /// Rotates the log file if it reaches a certain size
    async fn rotate_log_if_needed(file: &File, path: &Path) -> MonocoreResult<()> {
        let metadata = file.metadata().await?;
        if metadata.len() > Self::MAX_LOG_SIZE {
            // Ensure all data is written before rotation
            file.sync_all().await?;

            // Rotate old log file if it exists
            let backup_path = path.with_extension(format!(
                "{}.old",
                path.extension().unwrap_or_default().to_str().unwrap_or("")
            ));

            // Remove old backup if it exists
            if backup_path.exists() {
                if let Err(e) = fs::remove_file(&backup_path).await {
                    warn!("Failed to remove old backup log file: {}", e);
                }
            }

            // Rename current log to backup
            if let Err(e) = fs::rename(path, &backup_path).await {
                warn!("Failed to rotate log file: {}", e);
            }
        }
        Ok(())
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
            let state = self.state.lock().await;
            let service = state.get_service();
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
                &local_only_json,
                &group_ip_json,
                &rootfs_path,
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
            let mut file = match Self::create_log_file(&stdout_path).await {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create stdout log file: {}", e);
                    return;
                }
            };

            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Check and rotate if needed
                if let Err(e) = Self::rotate_log_if_needed(&file, &stdout_path).await {
                    error!("Failed to rotate stdout log: {}", e);
                }

                // Reopen file if it was rotated
                if !stdout_path.exists() {
                    file = match Self::create_log_file(&stdout_path).await {
                        Ok(f) => f,
                        Err(e) => {
                            error!("Failed to create new stdout log file after rotation: {}", e);
                            return;
                        }
                    };
                }

                // Format the log entry with timestamp in standard log format
                let now = chrono::Utc::now();
                let formatted_line = format!(
                    "{} INFO [{}] {}\n",
                    now.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                    service_name,
                    line
                );

                if let Err(e) = file.write_all(formatted_line.as_bytes()).await {
                    error!("Failed to write to stdout log: {}", e);
                }

                if let Err(e) = file.flush().await {
                    error!("Failed to flush stdout log: {}", e);
                }
            }
        });

        // Handle stderr
        let stderr = child.stderr.take().unwrap();
        let stderr_path = self.stderr_log_path.clone();
        let service_name = self.state.lock().await.get_service().get_name().to_string();
        let stderr_handle = tokio::spawn(async move {
            let mut file = match Self::create_log_file(&stderr_path).await {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create stderr log file: {}", e);
                    return;
                }
            };

            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Check and rotate if needed
                if let Err(e) = Self::rotate_log_if_needed(&file, &stderr_path).await {
                    error!("Failed to rotate stderr log: {}", e);
                }

                // Reopen file if it was rotated
                if !stderr_path.exists() {
                    file = match Self::create_log_file(&stderr_path).await {
                        Ok(f) => f,
                        Err(e) => {
                            error!("Failed to create new stderr log file after rotation: {}", e);
                            return;
                        }
                    };
                }

                // Format the log entry with timestamp in standard log format
                let now = chrono::Utc::now();
                let formatted_line = format!(
                    "{} ERROR [{}] {}\n",
                    now.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                    service_name,
                    line
                );

                if let Err(e) = file.write_all(formatted_line.as_bytes()).await {
                    error!("Failed to write to stderr log: {}", e);
                }
                if let Err(e) = file.flush().await {
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
}

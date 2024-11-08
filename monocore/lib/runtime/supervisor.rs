use std::{
    env,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{self, Stdio},
};

use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::broadcast,
    task::JoinHandle,
    time,
};

use tracing::{error, info, warn};

use crate::{
    config::{Group, Service},
    runtime::MicroVmStatus,
    utils::{MICROVM_LOG_DIR, MICROVM_STATE_DIR},
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
    state: MicroVmState,

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

    /// Creates a new  instance.
    ///
    /// # Arguments
    ///
    /// * `service` - The service configuration to supervise
    /// * `group` - The group configuration the service belongs to
    /// * `rootfs_path` - Path to the root filesystem for the micro VM
    pub async fn new(
        service: Service,
        group: Group,
        rootfs_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        // Generate unique IDs for the files
        let service_name = service.get_name();

        // Create paths with service name for better identification
        let runtime_state_path =
            MICROVM_STATE_DIR.join(format!("{}-{}.json", service_name, process::id()));
        let stdout_log_path = MICROVM_LOG_DIR.join(format!("{}.stdout.log", service_name));
        let stderr_log_path = MICROVM_LOG_DIR.join(format!("{}.stderr.log", service_name));

        // Create directories with proper permissions
        for dir in [&*MICROVM_STATE_DIR, &*MICROVM_LOG_DIR] {
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
            state: MicroVmState::new(service, group, rootfs_path),
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
        self.state.set_status(MicroVmStatus::Starting);
        self.save_runtime_state().await?;

        let current_exe = env::current_exe()?;

        // Serialize the service and group
        let service_json = serde_json::to_string(self.state.get_service())?;
        let env_pairs = self
            .state
            .get_service()
            .get_group_env(self.state.get_group())?;
        let env_json = serde_json::to_string(&env_pairs)?;
        let local_only_json = serde_json::to_string(self.state.get_group().get_local_only())?;
        let rootfs_path = self.state.get_rootfs_path().to_str().unwrap();

        // Start the micro VM sub process
        let mut child = Command::new(current_exe)
            .args([
                "--run-microvm",
                &service_json,
                &env_json,
                &local_only_json,
                rootfs_path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Set the status and PID of the micro VM process
        self.state.set_status(MicroVmStatus::Started);
        self.state.set_pid(child.id());
        self.save_runtime_state().await?;

        // Handle stdout
        let stdout = child.stdout.take().unwrap();
        let stdout_path = self.stdout_log_path.clone();
        let service_name = self.state.get_service().get_name().to_string();
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
        let service_name = self.state.get_service().get_name().to_string();
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

        // Handle process lifecycle
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let runtime_state_path = self.runtime_state_path.clone();
        let handle = tokio::spawn(async move {
            let result = tokio::select! {
                _ = shutdown_rx.recv() => {
                    // Received shutdown signal, terminate the process
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

            // Ensure log tasks are cleaned up
            stdout_handle.abort();
            stderr_handle.abort();

            result
        });

        Ok(handle)
    }

    /// Stops the supervised micro VM sub process.
    ///
    /// Sends a shutdown signal to the process and waits for it to terminate.
    pub async fn stop(&mut self) -> MonocoreResult<()> {
        self.state.set_status(MicroVmStatus::Stopping);
        self.save_runtime_state().await?;

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

        self.state
            .set_status(MicroVmStatus::Stopped { exit_code: 0 });
        Ok(())
    }

    /// Saves the current runtime state to disk.
    ///
    /// The state is saved to a JSON file at the path specified by `runtime_state_path`.
    pub async fn save_runtime_state(&self) -> MonocoreResult<()> {
        let state_json = serde_json::to_string_pretty(&self.state)?;
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

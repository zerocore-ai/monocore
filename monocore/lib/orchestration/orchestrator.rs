use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command as StdCommand,
};

use tokio::{
    fs,
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};
use tracing::{error, info};

use crate::{
    config::{Monocore, Service},
    runtime::MicroVmState,
    utils::MICROVM_STATE_DIR,
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The orchestrator of the monocore services.
pub struct Orchestrator {
    /// The monocore configuration.
    config: Monocore,

    /// The path to the root filesystem.
    rootfs_path: PathBuf,

    /// The path to the supervisor binary.
    supervisor_path: PathBuf,

    /// Map of running services and their process IDs.
    running_services: HashMap<String, u32>,
}

/// Status information for a service.
#[derive(Debug)]
pub struct ServiceStatus {
    /// The name of the service.
    pub name: String,

    /// The process ID of the service.
    pub pid: Option<u32>,

    /// The current state of the service.
    pub state: MicroVmState,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Orchestrator {
    /// Creates a new orchestrator.
    pub async fn new(
        config: Monocore,
        rootfs_path: impl AsRef<Path>,
        supervisor_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        // Ensure the state directory exists
        fs::create_dir_all(&*MICROVM_STATE_DIR).await?;

        // Verify supervisor binary exists
        let supervisor_path = supervisor_path.as_ref().to_path_buf();
        if !supervisor_path.exists() {
            return Err(MonocoreError::SupervisorBinaryNotFound(
                supervisor_path.display().to_string(),
            ));
        }

        Ok(Self {
            config,
            rootfs_path: rootfs_path.as_ref().to_path_buf(),
            supervisor_path,
            running_services: HashMap::new(),
        })
    }

    /// Starts services based on the configuration.
    /// If service_name is provided, starts only that service. Otherwise, starts all services.
    pub async fn up(&mut self, service_name: Option<&str>) -> MonocoreResult<()> {
        let services_to_start: Vec<Service> = match service_name {
            Some(name) => {
                let service = self
                    .config
                    .get_services()
                    .iter()
                    .find(|s| s.get_name() == name)
                    .ok_or_else(|| MonocoreError::ServiceNotFound(name.to_string()))?;
                vec![service.clone()]
            }
            None => self.config.get_services().to_vec(),
        };

        for service in services_to_start {
            self.start_service(&service).await?;
        }

        Ok(())
    }

    /// Stops running services.
    /// If service_name is provided, stops only that service. Otherwise, stops all services.
    pub async fn down(&mut self, service_name: Option<&str>) -> MonocoreResult<()> {
        let services_to_stop: Vec<String> = match service_name {
            Some(name) => vec![name.to_string()],
            None => self.running_services.keys().cloned().collect(),
        };

        for service_name in services_to_stop {
            if let Some(pid) = self.running_services.remove(&service_name) {
                // Check if process is still running before trying to stop it
                if is_process_running(pid) {
                    if let Err(e) = self.stop_service(pid).await {
                        error!("Failed to stop service {}: {}", service_name, e);
                    }
                } else {
                    info!(
                        "Service {} (PID {}) is no longer running",
                        service_name, pid
                    );
                }
            }
        }

        Ok(())
    }

    /// Gets the status of all services.
    pub async fn status(&self) -> MonocoreResult<Vec<ServiceStatus>> {
        let mut statuses = Vec::new();

        // Ensure directory exists before reading
        if !fs::try_exists(&*MICROVM_STATE_DIR).await? {
            fs::create_dir_all(&*MICROVM_STATE_DIR).await?;
            return Ok(statuses);
        }

        // Read all state files from the state directory
        let mut dir = fs::read_dir(&*MICROVM_STATE_DIR).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                match fs::read_to_string(&path).await {
                    Ok(contents) => match serde_json::from_str::<MicroVmState>(&contents) {
                        Ok(state) => {
                            statuses.push(ServiceStatus {
                                name: state.get_service().get_name().to_string(),
                                pid: *state.get_pid(),
                                state,
                            });
                        }
                        Err(e) => {
                            error!("Failed to parse state file {:?}: {}", path, e);
                            continue;
                        }
                    },
                    Err(e) => {
                        error!("Failed to read state file {:?}: {}", path, e);
                        continue;
                    }
                }
            }
        }

        Ok(statuses)
    }

    /// Starts a single service.
    async fn start_service(&mut self, service: &Service) -> MonocoreResult<()> {
        if self.running_services.contains_key(service.get_name()) {
            info!("Service {} is already running", service.get_name());
            return Ok(());
        }

        let group = self.config.get_group_for_service(service)?;

        let service_json = serde_json::to_string(service)?;
        let group_json = serde_json::to_string(group)?;

        if !fs::try_exists(&self.rootfs_path).await? {
            return Err(MonocoreError::RootFsPathNotFound(
                self.rootfs_path.display().to_string(),
            ));
        }

        // Use the supervisor binary path and pipe stdout/stderr
        let child = Command::new(&self.supervisor_path)
            .arg("run-supervisor")
            .args([
                &service_json,
                &group_json,
                self.rootfs_path.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let pid = child
            .id()
            .ok_or_else(|| MonocoreError::ProcessIdNotFound(service.get_name().to_string()))?;

        self.running_services
            .insert(service.get_name().to_string(), pid);

        info!("Started service {} with PID {}", service.get_name(), pid);

        // Spawn tasks to handle stdout and stderr
        let service_name = service.get_name().to_string();
        self.spawn_output_handler(child, service_name);

        Ok(())
    }

    /// Spawns tasks to handle the stdout and stderr of a child process
    fn spawn_output_handler(&self, mut child: Child, service_name: String) {
        // Handle stdout
        if let Some(stdout) = child.stdout.take() {
            let service_name = service_name.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[{}/stdout] {}", service_name, line);
                }
            });
        }

        // Handle stderr
        if let Some(stderr) = child.stderr.take() {
            let service_name = service_name.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    error!("[{}/stderr] {}", service_name, line);
                }
            });
        }

        // Spawn a task to wait for the child process
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => info!("Service {} exited with status: {}", service_name, status),
                Err(e) => error!("Failed to wait for service {}: {}", service_name, e),
            }
        });
    }

    /// Stops a service by its process ID.
    async fn stop_service(&self, pid: u32) -> MonocoreResult<()> {
        Command::new("kill")
            .arg(pid.to_string())
            .spawn()?
            .wait()
            .await?;

        Ok(())
    }
}

/// Checks if a process with the given PID is still running
fn is_process_running(pid: u32) -> bool {
    // On Unix systems, sending signal 0 checks process existence without actually sending a signal
    StdCommand::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map_or(false, |status| status.success())
}

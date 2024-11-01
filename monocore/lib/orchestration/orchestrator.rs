use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime},
};

use getset::Getters;
use tokio::{
    fs::{self, DirEntry},
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time,
};
use tracing::{error, info, warn};

use crate::{
    config::{Monocore, Service, DEFAULT_LOG_MAX_AGE},
    runtime::MicroVmState,
    utils::{MICROVM_LOG_DIR, MICROVM_STATE_DIR},
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The Orchestrator manages the lifecycle of monocore services, handling their startup, shutdown,
/// and monitoring. It coordinates multiple supervised services and provides status information
/// about their operation. It also manages log file cleanup based on configured policies.
pub struct Orchestrator {
    /// The monocore configuration.
    config: Monocore,

    /// The path to the root filesystem.
    rootfs_path: PathBuf,

    /// The path to the supervisor binary.
    supervisor_path: PathBuf,

    /// Map of running services and their process IDs.
    running_services: HashMap<String, u32>,

    /// Configuration for log retention and cleanup
    log_retention_policy: LogRetentionPolicy,
}

/// Configuration for managing log file retention and cleanup in the orchestrator.
///
/// This configuration controls:
/// - How long log files are retained before being eligible for deletion
/// - Whether cleanup happens automatically during service lifecycle operations
#[derive(Debug, Clone)]
pub struct LogRetentionPolicy {
    /// Maximum age of log files before they are eligible for deletion.
    /// Files older than this duration will be removed during cleanup operations.
    max_age: Duration,

    /// Whether to automatically clean up logs during service lifecycle operations (up/down).
    /// When true, old log files will be cleaned up during service start and stop operations.
    /// When false, cleanup must be triggered manually via `cleanup_old_logs()`.
    auto_cleanup: bool,
}

/// Status information for a service.
#[derive(Debug, Getters)]
#[getset(get = "pub with_prefix")]
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
    /// Creates a new Orchestrator instance with custom log retention policy, allowing
    /// fine-grained control over how service logs are managed.
    pub async fn with_log_retention_policy(
        config: Monocore,
        rootfs_path: impl AsRef<Path>,
        supervisor_path: impl AsRef<Path>,
        log_retention_policy: LogRetentionPolicy,
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
            log_retention_policy,
        })
    }

    /// Creates a new Orchestrator instance with default log retention policy.
    pub async fn new(
        config: Monocore,
        rootfs_path: impl AsRef<Path>,
        supervisor_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        Self::with_log_retention_policy(
            config,
            rootfs_path,
            supervisor_path,
            LogRetentionPolicy::default(),
        )
        .await
    }

    /// Starts services according to the configuration. Can start either a single specified
    /// service or all configured services. Performs log cleanup if automatic cleanup is enabled.
    ///
    /// The service_name parameter controls which services to start:
    /// - When None: starts all services defined in the configuration
    /// - When Some(name): starts only the specified service, returning an error if the service is not found
    pub async fn up(&mut self, service_name: Option<&str>) -> MonocoreResult<()> {
        if self.log_retention_policy.auto_cleanup {
            if let Err(e) = self.cleanup_old_logs().await {
                warn!("Failed to clean up old logs during startup: {}", e);
            }
        }

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

    /// Stops running services, either a single specified service or all running services.
    /// Sends SIGTERM to processes and waits for graceful shutdown. Performs log cleanup
    /// if automatic cleanup is enabled.
    ///
    /// The service_name parameter controls which services to stop:
    /// - When None: stops all currently running services
    /// - When Some(name): stops only the specified service, doing nothing if the service isn't running
    pub async fn down(&mut self, service_name: Option<&str>) -> MonocoreResult<()> {
        if self.log_retention_policy.auto_cleanup {
            if let Err(e) = self.cleanup_old_logs().await {
                warn!("Failed to clean up old logs during shutdown: {}", e);
            }
        }

        let services_to_stop: Vec<String> = match service_name {
            Some(name) => vec![name.to_string()],
            None => self.running_services.keys().cloned().collect(),
        };

        for service_name in services_to_stop {
            if let Some(pid) = self.running_services.remove(&service_name) {
                info!(
                    "Stopping supervisor for service {} (PID {})",
                    service_name, pid
                );

                // Send SIGTERM signal once
                if let Err(e) = self.stop_service(pid).await {
                    error!("Failed to send SIGTERM to service {}: {}", service_name, e);
                    continue;
                }

                // Wait for process to exit gracefully with timeout
                let mut attempts = 5;
                while attempts > 0 && self.is_process_running(pid).await {
                    time::sleep(Duration::from_secs(2)).await;
                    attempts -= 1;
                }

                // Only log warning if process is still running after timeout
                if self.is_process_running(pid).await {
                    warn!(
                        "Service {} (PID {}) did not exit within timeout period",
                        service_name, pid
                    );
                }
            }
        }

        Ok(())
    }

    /// Retrieves the current status of all services, including their process IDs and state
    /// information. Also identifies and cleans up stale state files for processes that are
    /// no longer running.
    pub async fn status(&self) -> MonocoreResult<Vec<ServiceStatus>> {
        let mut statuses = Vec::new();
        let mut stale_files = Vec::new();

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
                            // Check if the process is still running
                            if let Some(pid) = state.get_pid() {
                                if !self.is_process_running(*pid).await {
                                    stale_files.push(path);
                                    continue;
                                }
                            }

                            statuses.push(ServiceStatus {
                                name: state.get_service().get_name().to_string(),
                                pid: *state.get_pid(),
                                state,
                            });
                        }
                        Err(e) => {
                            error!("Failed to parse state file {:?}: {}", path, e);
                            stale_files.push(path);
                        }
                    },
                    Err(e) => {
                        error!("Failed to read state file {:?}: {}", path, e);
                        stale_files.push(path);
                    }
                }
            }
        }

        // Clean up stale files
        for path in stale_files {
            if let Err(e) = fs::remove_file(&path).await {
                warn!("Failed to remove stale state file {:?}: {}", path, e);
            }
        }

        Ok(statuses)
    }

    /// Starts a single service by spawning a supervisor process and setting up output handling.
    /// The supervisor manages the actual service process and maintains its state.
    async fn start_service(&mut self, service: &Service) -> MonocoreResult<()> {
        if self.running_services.contains_key(service.get_name()) {
            info!("Service {} is already running", service.get_name());
            return Ok(());
        }

        let group = self.config.get_group_for_service(service)?;

        let service_json = serde_json::to_string(service)?;
        let group_json = serde_json::to_string(group)?;

        // Use the supervisor binary path and pipe stdout/stderr
        let child = Command::new(&self.supervisor_path)
            .arg("run-supervisor")
            .args([
                &service_json,
                &group_json,
                self.rootfs_path.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let pid = child
            .id()
            .ok_or_else(|| MonocoreError::ProcessIdNotFound(service.get_name().to_string()))?;

        self.running_services
            .insert(service.get_name().to_string(), pid);

        info!(
            "Started supervisor for service {} with PID {}",
            service.get_name(),
            pid
        );

        // Spawn tasks to handle stdout and stderr
        let service_name = service.get_name().to_string();
        self.spawn_output_handler(child, service_name);

        Ok(())
    }

    /// Sets up asynchronous handlers for process output streams, capturing stdout and stderr
    /// from the supervised process and logging them appropriately.
    fn spawn_output_handler(&self, mut child: Child, service_name: String) {
        // Handle stdout
        match child.stdout.take() {
            Some(stdout) => {
                let stdout_service_name = service_name.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        info!("[{}/stdout] {}", stdout_service_name, line);
                    }
                });
            }
            None => {
                warn!(
                    "Failed to capture stdout for supervisor of service {}",
                    service_name
                );
            }
        }

        // Handle stderr
        match child.stderr.take() {
            Some(stderr) => {
                let stderr_service_name = service_name.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        error!("[{}/stderr] {}", stderr_service_name, line);
                    }
                });
            }
            None => {
                warn!(
                    "Failed to capture stderr for supervisor of service {}",
                    service_name
                );
            }
        }

        // Wait for the child process
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => {
                    info!(
                        "Service supervisor for {} exited with status: {}",
                        service_name, status
                    );
                }
                Err(e) => {
                    error!("Failed to wait for service {}: {}", service_name, e);
                }
            }
        });
    }

    /// Sends a termination signal to a service process identified by its PID.
    async fn stop_service(&self, pid: u32) -> MonocoreResult<()> {
        // Send SIGTERM only once
        Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .output()
            .await
            .map_err(|e| MonocoreError::ProcessKillError(e.to_string()))?;

        Ok(())
    }

    /// Verifies if a process with the given PID is still active in the system.
    async fn is_process_running(&self, pid: u32) -> bool {
        Command::new("kill")
            .arg("-0") // Only check process existence
            .arg(pid.to_string())
            .output()
            .await
            .map_or(false, |output| output.status.success())
    }

    /// Performs cleanup of old log files based on the configured maximum age. Removes
    /// files that exceed the age threshold and logs the cleanup activity.
    pub async fn cleanup_old_logs(&self) -> MonocoreResult<()> {
        // Ensure log directory exists before attempting cleanup
        if !fs::try_exists(&*MICROVM_LOG_DIR).await? {
            fs::create_dir_all(&*MICROVM_LOG_DIR).await?;
            return Ok(());
        }

        let now = SystemTime::now();
        let mut cleaned_files = 0;

        let mut entries = fs::read_dir(&*MICROVM_LOG_DIR).await?;
        while let Some(entry) = entries.next_entry().await? {
            if self
                .should_delete_log(&entry, now, self.log_retention_policy.max_age)
                .await?
            {
                if let Err(e) = fs::remove_file(entry.path()).await {
                    warn!(
                        "Failed to remove old log file {}: {}",
                        entry.path().display(),
                        e
                    );
                } else {
                    cleaned_files += 1;
                }
            }
        }

        if cleaned_files > 0 {
            info!("Cleaned up {} old log files", cleaned_files);
        }

        Ok(())
    }

    /// Evaluates whether a specific log file should be deleted based on its age and
    /// file extension. Only processes files with .log or .log.old extensions.
    async fn should_delete_log(
        &self,
        entry: &DirEntry,
        now: SystemTime,
        max_age: Duration,
    ) -> MonocoreResult<bool> {
        // Only process .log and .log.old files
        let is_log = entry
            .path()
            .extension()
            .map_or(false, |ext| ext == "log" || ext == "old");

        if !is_log {
            return Ok(false);
        }

        let metadata = entry.metadata().await?;
        let modified = metadata.modified()?;

        // Calculate file age
        let age = now
            .duration_since(modified)
            .unwrap_or_else(|_| Duration::from_secs(0));

        Ok(age > max_age)
    }
}

impl Default for LogRetentionPolicy {
    /// Creates a default configuration that:
    /// - Keeps logs for 7 days
    /// - Enables automatic cleanup during service operations
    fn default() -> Self {
        Self {
            max_age: DEFAULT_LOG_MAX_AGE,
            auto_cleanup: true,
        }
    }
}

impl LogRetentionPolicy {
    /// Creates a new log retention policy with custom settings.
    pub fn new(max_age: Duration, auto_cleanup: bool) -> Self {
        Self {
            max_age,
            auto_cleanup,
        }
    }

    /// Creates a new policy that retains logs for the specified number of hours.
    pub fn with_max_age_hours(hours: u64) -> Self {
        Self {
            max_age: Duration::from_secs(hours * 60 * 60),
            auto_cleanup: true,
        }
    }

    /// Creates a new policy that retains logs for the specified number of days.
    pub fn with_max_age_days(days: u64) -> Self {
        Self {
            max_age: Duration::from_secs(days * 24 * 60 * 60),
            auto_cleanup: true,
        }
    }

    /// Creates a new policy that retains logs for the specified number of weeks.
    pub fn with_max_age_weeks(weeks: u64) -> Self {
        Self {
            max_age: Duration::from_secs(weeks * 7 * 24 * 60 * 60),
            auto_cleanup: true,
        }
    }

    /// Creates a new policy that retains logs for the specified number of months.
    /// Note: Uses a 30-day approximation for months.
    pub fn with_max_age_months(months: u64) -> Self {
        Self {
            max_age: Duration::from_secs(months * 30 * 24 * 60 * 60),
            auto_cleanup: true,
        }
    }
}

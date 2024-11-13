use std::{
    collections::{BTreeSet, HashMap, HashSet},
    net::Ipv4Addr,
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
    oci::rootfs,
    runtime::MicroVmState,
    utils::{MERGED_SUBDIR, MONOCORE_LOG_DIR, MONOCORE_STATE_DIR, REFERENCE_SUBDIR},
    MonocoreError, MonocoreResult,
};

use super::utils::{self, LoadedState};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The Orchestrator manages the lifecycle of monocore services, handling their startup, shutdown,
/// and monitoring. It coordinates multiple supervised services and provides status information
/// about their operation. It also manages log file cleanup based on configured policies.
pub struct Orchestrator {
    /// The monocore configuration.
    config: Monocore,

    /// The path to the directory containing service rootfs directories
    services_rootfs_dir: PathBuf,

    /// The path to the supervisor executable.
    supervisor_exe_path: PathBuf,

    /// Map of running services and their process IDs.
    running_services: HashMap<String, u32>,

    /// Configuration for log retention and cleanup
    log_retention_policy: LogRetentionPolicy,

    /// Maps group name to assigned IP
    assigned_ips: HashMap<String, Ipv4Addr>,

    /// Tracks used last octets for IP assignment
    used_ips: BTreeSet<u8>,
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
    /// Creates a new Orchestrator instance with custom log retention policy
    pub async fn with_log_retention_policy(
        services_rootfs_dir: impl AsRef<Path>,
        supervisor_exe_path: impl AsRef<Path>,
        log_retention_policy: LogRetentionPolicy,
    ) -> MonocoreResult<Self> {
        // Ensure the state directory exists
        fs::create_dir_all(&*MONOCORE_STATE_DIR).await?;

        // Verify supervisor binary exists
        let supervisor_exe_path = supervisor_exe_path.as_ref().to_path_buf();
        if !supervisor_exe_path.exists() {
            return Err(MonocoreError::SupervisorBinaryNotFound(
                supervisor_exe_path.display().to_string(),
            ));
        }

        Ok(Self {
            config: Monocore::default(),
            services_rootfs_dir: services_rootfs_dir.as_ref().to_path_buf(),
            supervisor_exe_path,
            running_services: HashMap::new(),
            log_retention_policy,
            assigned_ips: HashMap::new(),
            used_ips: BTreeSet::new(),
        })
    }

    /// Creates a new Orchestrator instance with default log retention policy
    pub async fn new(
        services_rootfs_dir: impl AsRef<Path>,
        supervisor_exe_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        Self::with_log_retention_policy(
            services_rootfs_dir,
            supervisor_exe_path,
            LogRetentionPolicy::default(),
        )
        .await
    }

    /// Starts or updates services according to the provided configuration.
    /// Merges the new config with existing config and starts/restarts changed services.
    pub async fn up(&mut self, new_config: Monocore) -> MonocoreResult<()> {
        if self.log_retention_policy.auto_cleanup {
            if let Err(e) = self.cleanup_old_logs().await {
                warn!("Failed to clean up old logs during startup: {}", e);
            }
        }

        // Clone current config to avoid borrowing issues
        let current_config = self.config.clone();

        // Get the services that changed or were added
        let changed_services: HashSet<_> = current_config
            .get_changed_services(&new_config)
            .into_iter()
            .map(|s| s.get_name().to_string())
            .collect();

        // Merge the configurations
        self.config = current_config.merge(&new_config)?;

        // Get ordered list of changed services based on dependencies
        let ordered_services: Vec<_> = self
            .config
            .get_ordered_services()
            .into_iter()
            .filter(|s| changed_services.contains(s.get_name()))
            .collect();

        // Clone the ordered services to avoid borrow issues
        let ordered_services: Vec<_> = ordered_services.into_iter().cloned().collect();

        // Start/restart changed services in dependency order
        for service in ordered_services {
            // Stop the service if it's running
            if let Some(pid) = self.running_services.get(service.get_name()) {
                let pid = *pid; // Copy the pid to avoid borrow issues
                self.stop_service(pid).await?;
                self.running_services.remove(service.get_name());
            }

            // Start the service with new configuration
            self.start_service(&service).await?;
        }

        Ok(())
    }

    /// Stops running services and removes them from the configuration.
    /// When service_name is None, stops and removes all services.
    pub async fn down(&mut self, service_name: Option<&str>) -> MonocoreResult<()> {
        if self.log_retention_policy.auto_cleanup {
            if let Err(e) = self.cleanup_old_logs().await {
                warn!("Failed to clean up old logs during shutdown: {}", e);
            }
        }

        // Get the services to stop
        let services_to_stop: HashSet<String> = match service_name {
            Some(name) => vec![name.to_string()].into_iter().collect(),
            None => self.running_services.keys().cloned().collect(),
        };

        // Get all services in dependency order (reversed for shutdown)
        let ordered_services: Vec<_> = self
            .config
            .get_ordered_services()
            .into_iter()
            .filter(|s| services_to_stop.contains(s.get_name()))
            .rev() // Reverse the order for shutdown
            .collect();

        // Clone the ordered services to avoid borrow issues
        let ordered_services: Vec<_> = ordered_services.into_iter().cloned().collect();

        // Clone ordered_services before using it
        let services_for_groups = ordered_services.clone();

        // Stop services in reverse dependency order
        for service in ordered_services {
            let service_name = service.get_name();

            // Stop the service if it's running
            if let Some(pid) = self.running_services.remove(service_name) {
                info!(
                    "Stopping supervisor for service {} (PID {})",
                    service_name, pid
                );

                if let Err(e) = self.stop_service(pid).await {
                    error!("Failed to send SIGTERM to service {}: {}", service_name, e);
                    continue;
                }

                // Wait for process to exit gracefully with timeout
                let mut attempts = 5;
                while attempts > 0 && utils::is_process_running(pid).await {
                    time::sleep(Duration::from_secs(2)).await;
                    attempts -= 1;
                }

                if utils::is_process_running(pid).await {
                    warn!(
                        "Service {} (PID {}) did not exit within timeout period",
                        service_name, pid
                    );
                }
            }
        }

        // Convert HashSet back to Vec for remove_services
        let services_to_stop: Vec<_> = services_to_stop.into_iter().collect();

        // Remove services from config in place
        self.config.remove_services(Some(&services_to_stop));

        // Get groups that will have no running services after shutdown
        let mut empty_groups = HashSet::new();
        for service in services_for_groups.iter() {
            let group_name = service.get_group().unwrap_or_default();
            let group_has_other_services = self.running_services.keys().any(|name| {
                name != service.get_name()
                    && self
                        .config
                        .get_service(name)
                        .map(|s| s.get_group().unwrap_or_default() == group_name)
                        .unwrap_or(false)
            });
            if !group_has_other_services {
                empty_groups.insert(group_name);
            }
        }

        // Release IPs for groups with no running services
        for group_name in empty_groups {
            self.release_group_ip(group_name);
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
        if !fs::try_exists(&*MONOCORE_STATE_DIR).await? {
            fs::create_dir_all(&*MONOCORE_STATE_DIR).await?;
            return Ok(statuses);
        }

        // Read all state files from the state directory
        let mut dir = fs::read_dir(&*MONOCORE_STATE_DIR).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                match fs::read_to_string(&path).await {
                    Ok(contents) => match serde_json::from_str::<MicroVmState>(&contents) {
                        Ok(state) => {
                            // Check if the process is still running
                            if let Some(pid) = state.get_pid() {
                                if !utils::is_process_running(*pid).await {
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

    /// Starts a single service by spawning a supervisor process.
    /// Handles IP assignment for the service's group and passes the IP through
    /// to the supervisor and microvm processes.
    async fn start_service(&mut self, service: &Service) -> MonocoreResult<()> {
        if self.running_services.contains_key(service.get_name()) {
            info!("Service {} is already running", service.get_name());
            return Ok(());
        }

        // Get service-specific rootfs path
        let service_rootfs = self.services_rootfs_dir.join(service.get_name());

        // If service rootfs doesn't exist, try to create it from reference
        if !service_rootfs.exists() {
            info!(
                "Service rootfs not found at {}, attempting to create from reference",
                service_rootfs.display()
            );

            // Get base image name from service config
            let base_image = service.get_base().ok_or_else(|| {
                MonocoreError::ConfigValidation(format!(
                    "Service {} has no base image specified",
                    service.get_name()
                ))
            })?;

            // Construct path to reference rootfs
            let reference_rootfs = self
                .services_rootfs_dir
                .parent() // Go up from service dir
                .ok_or_else(|| {
                    MonocoreError::PathNotFound(
                        "Invalid services rootfs path - no parent directory found".to_string(),
                    )
                })?
                .join(REFERENCE_SUBDIR)
                .join(crate::utils::parse_image_ref(base_image)?.2) // Convert repo:tag to repo__tag
                .join(MERGED_SUBDIR);

            if !reference_rootfs.exists() {
                return Err(MonocoreError::RootfsNotFound(format!(
                    "Reference rootfs not found at {}",
                    reference_rootfs.display()
                )));
            }

            // Create parent directories
            fs::create_dir_all(service_rootfs.parent().unwrap()).await?;

            // Copy reference rootfs to service rootfs
            info!(
                "Copying reference rootfs from {} to {}",
                reference_rootfs.display(),
                service_rootfs.display()
            );

            rootfs::copy(&reference_rootfs, &service_rootfs, false).await?;
        }

        // Get group and prepare configuration data
        let group = self.config.get_group_for_service(service)?;
        let group_name = group.get_name().to_string();

        // Serialize configuration before IP assignment
        let service_json = serde_json::to_string(service)?;
        let group_json = serde_json::to_string(&group)?;

        // Assign IP address to the group
        let group_ip = self.assign_group_ip(&group_name)?;
        let group_ip_json = serde_json::to_string(&group_ip)?;

        // Start the supervisor process with service-specific rootfs
        let child = Command::new(&self.supervisor_exe_path)
            .arg("--run-supervisor")
            .args([
                &service_json,
                &group_json,
                &group_ip_json,
                service_rootfs.to_str().unwrap(),
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

    /// Performs cleanup of old log files based on the configured maximum age. Removes
    /// files that exceed the age threshold and logs the cleanup activity.
    pub async fn cleanup_old_logs(&self) -> MonocoreResult<()> {
        // Ensure log directory exists before attempting cleanup
        if !fs::try_exists(&*MONOCORE_LOG_DIR).await? {
            fs::create_dir_all(&*MONOCORE_LOG_DIR).await?;
            return Ok(());
        }

        let now = SystemTime::now();
        let mut cleaned_files = 0;

        let mut entries = fs::read_dir(&*MONOCORE_LOG_DIR).await?;
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
        let path = entry.path();
        let is_log = path
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

    /// Creates a new Orchestrator instance from existing state files with custom log retention policy.
    /// This allows reconstructing the orchestrator's state from running services.
    ///
    /// ## Arguments
    /// * `rootfs_path` - Path to the root filesystem
    /// * `supervisor_exe_path` - Path to the supervisor executable
    /// * `log_retention_policy` - Configuration for log file retention and cleanup
    ///
    /// ## Example
    /// ```no_run
    /// use monocore::orchestration::{Orchestrator, LogRetentionPolicy};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let orchestrator = Orchestrator::load_with_log_retention_policy(
    ///     "/path/to/rootfs",
    ///     "/path/to/supervisor",
    ///     LogRetentionPolicy::with_max_age_days(7)
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn load_with_log_retention_policy(
        services_rootfs_dir: impl AsRef<Path>,
        supervisor_exe_path: impl AsRef<Path>,
        log_retention_policy: LogRetentionPolicy,
    ) -> MonocoreResult<Self> {
        // Ensure the state directory exists
        fs::create_dir_all(&*MONOCORE_STATE_DIR).await?;

        // Verify supervisor binary exists
        let supervisor_exe_path = supervisor_exe_path.as_ref().to_path_buf();
        if !supervisor_exe_path.exists() {
            return Err(MonocoreError::SupervisorBinaryNotFound(
                supervisor_exe_path.display().to_string(),
            ));
        }

        // Load state from files
        let state = utils::load_state_from_files(&MONOCORE_STATE_DIR).await?;

        // Create config from state
        let (
            config,
            LoadedState {
                running_services,
                assigned_ips,
                used_ips,
                ..
            },
        ) = utils::create_config_from_state(state)?;

        Ok(Self {
            config,
            services_rootfs_dir: services_rootfs_dir.as_ref().to_path_buf(),
            supervisor_exe_path,
            running_services,
            log_retention_policy,
            assigned_ips,
            used_ips,
        })
    }

    /// Creates a new Orchestrator instance from existing state files with default log retention policy.
    ///
    /// This is a convenience method that uses the default log retention policy (7 days, auto cleanup enabled).
    ///
    /// ## Arguments
    /// * `rootfs_path` - Path to the root filesystem
    /// * `supervisor_exe_path` - Path to the supervisor executable
    ///
    /// ## Returns
    /// A new Orchestrator instance initialized from existing state files
    ///
    /// ## Example
    /// ```no_run
    /// use monocore::orchestration::Orchestrator;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let orchestrator = Orchestrator::load(
    ///     "/path/to/rootfs",
    ///     "/path/to/supervisor"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn load(
        services_rootfs_dir: impl AsRef<Path>,
        supervisor_exe_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        Self::load_with_log_retention_policy(
            services_rootfs_dir,
            supervisor_exe_path,
            LogRetentionPolicy::default(),
        )
        .await
    }

    /// Assigns an IP address to a group from the 127.0.0.x range.
    /// Returns the existing IP if the group already has one assigned.
    ///
    /// The IP assignment follows these rules:
    /// - Uses addresses in the range 127.0.0.2 to 127.0.0.254
    /// - Skips 127.0.0.0, 127.0.0.1, and 127.0.0.255
    /// - Reuses IPs from terminated groups
    /// - Maintains consistent IP assignment for a group
    fn assign_group_ip(&mut self, group_name: &str) -> MonocoreResult<Option<Ipv4Addr>> {
        // Return existing IP if already assigned
        if let Some(ip) = self.assigned_ips.get(group_name) {
            return Ok(Some(*ip));
        }

        // Find first available last octet (2-254, skipping 0, 1, and 255)
        let last_octet = match (2..=254).find(|&n| !self.used_ips.contains(&n)) {
            Some(n) => n,
            None => return Ok(None), // No IPs available
        };

        let ip = Ipv4Addr::new(127, 0, 0, last_octet);
        self.used_ips.insert(last_octet);
        self.assigned_ips.insert(group_name.to_string(), ip);

        Ok(Some(ip))
    }

    /// Releases an IP address assigned to a group, making it available for reuse.
    /// This should be called when a group no longer has any running services.
    fn release_group_ip(&mut self, group_name: &str) {
        if let Some(ip) = self.assigned_ips.remove(group_name) {
            self.used_ips.remove(&ip.octets()[3]);
        }
    }

    /// Gets a reference to the map of running services and their supervisor PIDs
    pub fn get_running_services(&self) -> &HashMap<String, u32> {
        &self.running_services
    }

    /// Gets a service from the current configuration by name
    pub fn get_service(&self, name: &str) -> Option<&Service> {
        self.config.get_service(name)
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

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

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

use std::{
    collections::{BTreeSet, HashMap},
    net::Ipv4Addr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use getset::Getters;
use tokio::{
    fs::{self, DirEntry},
    process::Command,
};

use crate::{
    config::{Monocore, Service},
    runtime::MicroVmState,
    utils::{MONOCORE_LOG_DIR, MONOCORE_STATE_DIR},
    MonocoreError, MonocoreResult,
};

use super::{
    utils::{self, LoadedState},
    LogRetentionPolicy,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The Orchestrator manages the lifecycle of monocore services, handling their startup, shutdown,
/// and monitoring. It coordinates multiple supervised services and provides status information
/// about their operation. It also manages log file cleanup based on configured policies.
#[derive(Debug, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Orchestrator {
    /// The monocore configuration.
    pub(super) config: Monocore,

    /// The path to the directory containing rootfs directories and service configuration files
    pub(super) home_dir: PathBuf,

    /// The path to the supervisor executable.
    pub(super) supervisor_exe_path: PathBuf,

    /// Map of running services and their process IDs.
    pub(super) running_services: HashMap<String, u32>,

    /// Configuration for log retention and cleanup
    pub(super) log_retention_policy: LogRetentionPolicy,

    /// Maps group name to assigned IP
    pub(super) assigned_ips: HashMap<String, Ipv4Addr>,

    /// Tracks used last octets for IP assignment
    pub(super) used_ips: BTreeSet<u8>,
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
        home_dir: impl AsRef<Path>,
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
            home_dir: home_dir.as_ref().to_path_buf(),
            supervisor_exe_path,
            running_services: HashMap::new(),
            log_retention_policy,
            assigned_ips: HashMap::new(),
            used_ips: BTreeSet::new(),
        })
    }

    /// Creates a new Orchestrator instance with default log retention policy
    pub async fn new(
        home_dir: impl AsRef<Path>,
        supervisor_exe_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        Self::with_log_retention_policy(
            home_dir,
            supervisor_exe_path,
            LogRetentionPolicy::default(),
        )
        .await
    }

    /// Creates a new Orchestrator instance from existing state files with custom log retention policy.
    /// This allows reconstructing the orchestrator's state from running services.
    ///
    /// ## Arguments
    /// * `home_dir` - Path to the directory containing rootfs directories and service configuration files
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
        home_dir: impl AsRef<Path>,
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
            home_dir: home_dir.as_ref().to_path_buf(),
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
    /// * `home_dir` - Path to the directory containing rootfs directories and service configuration files
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
        home_dir: impl AsRef<Path>,
        supervisor_exe_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        Self::load_with_log_retention_policy(
            home_dir,
            supervisor_exe_path,
            LogRetentionPolicy::default(),
        )
        .await
    }

    /// Gets a service from the current configuration by name
    pub fn get_service(&self, name: &str) -> Option<&Service> {
        self.config.get_service(name)
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
                    tracing::warn!(
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
            tracing::info!("Cleaned up {} old log files", cleaned_files);
        }

        Ok(())
    }

    /// Sends a termination signal to a service process identified by its PID.
    pub(super) async fn stop_service(&self, pid: u32) -> MonocoreResult<()> {
        // Send SIGTERM only once
        Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .output()
            .await
            .map_err(|e| MonocoreError::ProcessKillError(e.to_string()))?;

        Ok(())
    }

    /// Evaluates whether a specific log file should be deleted based on its age and
    /// file extension. Only processes files with .log or .log.old extensions.
    pub(super) async fn should_delete_log(
        &self,
        entry: &DirEntry,
        now: SystemTime,
        max_age: Duration,
    ) -> MonocoreResult<bool> {
        // Only process .log and .log.old files
        let path = entry.path();
        let is_log = path
            .extension()
            .is_some_and(|ext| ext == "log" || ext == "old");

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

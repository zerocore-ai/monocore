use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

use crate::config::{Group, Service};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The state of the micro VM sub process.
#[derive(Debug, Clone, Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub with_prefix", set = "pub with_prefix")]
pub struct MicroVmState {
    /// The process ID of the micro VM sub process.
    pid: Option<u32>,

    /// The time the micro VM sub process was created.
    created_at: DateTime<Utc>,

    /// The time of the last modification of the micro VM sub process.
    modified_at: DateTime<Utc>,

    /// The service configuration of the micro VM sub process.
    service: Service,

    /// The group configuration the service belongs to.
    group: Group,

    /// The path to the rootfs of the micro VM OS.
    rootfs_path: PathBuf,

    /// The status of the micro VM sub process.
    status: MicroVmStatus,

    /// The metrics of the micro VM sub process.
    metrics: MicroVmMetrics,
    // /// The IP address of the group.
    // group_addr: Ip4Addr,
}

/// The status of the micro VM sub process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MicroVmStatus {
    /// The micro VM sub process is not started.
    Unstarted,

    /// The micro VM sub process is starting.
    Starting,

    /// The micro VM sub process is started.
    Started,

    /// The micro VM sub process is stopping.
    Stopping,

    /// The micro VM sub process is stopped.
    Stopped {
        /// The exit code of the micro VM sub process.
        exit_code: i32,
    },

    /// The micro VM sub process failed.
    Failed {
        /// The error that occurred.
        error: String,
    },
}

/// The metrics of the micro VM sub process.
#[derive(Debug, Clone, Default, Getters, Setters, PartialEq, Serialize, Deserialize)]
#[getset(get = "pub with_prefix", set = "pub with_prefix")]
pub struct MicroVmMetrics {
    /// The CPU usage of the micro VM.
    cpu_usage: f64,

    /// The memory usage of the micro VM.
    memory_usage: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MicroVmState {
    /// Creates a new micro VM state.
    pub fn new(service: Service, group: Group, rootfs_path: impl AsRef<Path>) -> Self {
        Self {
            pid: None,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            service,
            group,
            rootfs_path: rootfs_path.as_ref().into(),
            status: MicroVmStatus::Unstarted,
            metrics: MicroVmMetrics::default(),
        }
    }

    /// Saves the state to a file.
    pub async fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let serialized = serde_json::to_string(self)?;
        tokio::fs::write(path, serialized).await?;
        Ok(())
    }

    /// Loads the state from a file.
    pub async fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let data = tokio::fs::read_to_string(path).await?;
        let state = serde_json::from_str(&data)?;
        Ok(state)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_save_and_load_microvm_state() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let state_file = NamedTempFile::new()?;
        let state = MicroVmState::new(
            Service::default(),
            Group::builder().name("test-group").build(),
            temp_dir.path(),
        );

        // Save state
        state.save(state_file.path()).await?;

        // Load state
        let loaded_state = MicroVmState::load(state_file.path()).await?;

        // Compare states
        assert_eq!(state.get_pid(), loaded_state.get_pid());
        assert_eq!(state.get_created_at(), loaded_state.get_created_at());
        assert_eq!(state.get_modified_at(), loaded_state.get_modified_at());
        assert_eq!(state.get_service(), loaded_state.get_service());
        assert_eq!(state.get_rootfs_path(), loaded_state.get_rootfs_path());
        assert_eq!(state.get_status(), loaded_state.get_status());
        assert_eq!(state.get_metrics(), loaded_state.get_metrics());

        Ok(())
    }
}

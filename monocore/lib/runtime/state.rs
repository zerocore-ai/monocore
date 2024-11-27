use std::{
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use getset::{Getters, MutGetters, Setters};
use serde::{Deserialize, Serialize};

use crate::config::{Group, Service};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The state of the micro VM sub process.
#[derive(Debug, Clone, Getters, Setters, MutGetters, Serialize, Deserialize)]
#[getset(
    get = "pub with_prefix",
    set = "pub with_prefix",
    get_mut = "pub with_prefix"
)]
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

    /// The IP address of the group.
    group_ip: Option<Ipv4Addr>,
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

/// Metrics collected from a running micro VM process.
///
/// This struct contains various performance metrics that are collected in real-time
/// from the running micro VM process, including:
/// - CPU usage as a percentage (0-100)
/// - Memory usage in bytes
/// - Disk I/O statistics including read/write bytes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicroVmMetrics {
    /// CPU usage as a percentage (0-100).
    /// This represents the percentage of CPU time used by the micro VM process
    /// across all cores.
    cpu_usage: f32,

    /// Memory usage in bytes.
    /// This represents the current resident set size (RSS) of the micro VM process.
    memory_usage: u64,

    /// Number of bytes read from disk since last measurement.
    /// This is the delta of read operations between measurements.
    disk_read_bytes: u64,

    /// Number of bytes written to disk since last measurement.
    /// This is the delta of write operations between measurements.
    disk_write_bytes: u64,

    /// Total number of bytes read from disk since process start.
    /// This is the cumulative amount of data read by the process.
    total_disk_read_bytes: u64,

    /// Total number of bytes written to disk since process start.
    /// This is the cumulative amount of data written by the process.
    total_disk_write_bytes: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MicroVmState {
    /// Creates a new micro VM state.
    pub fn new(
        service: Service,
        group: Group,
        group_ip: Option<Ipv4Addr>,
        rootfs_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            pid: None,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            service,
            group,
            rootfs_path: rootfs_path.as_ref().to_path_buf(),
            status: MicroVmStatus::Unstarted,
            metrics: MicroVmMetrics::new(),
            group_ip,
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

impl MicroVmMetrics {
    /// Creates a new `MicroVmMetrics` instance with all metrics initialized to zero.
    pub fn new() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            total_disk_read_bytes: 0,
            total_disk_write_bytes: 0,
        }
    }

    /// Sets the current CPU usage percentage.
    ///
    /// # Arguments
    /// * `usage` - CPU usage as a percentage between 0.0 and 100.0
    pub fn set_cpu_usage(&mut self, usage: f32) {
        self.cpu_usage = usage;
    }

    /// Sets the current memory usage in bytes.
    ///
    /// # Arguments
    /// * `usage` - Memory usage in bytes
    pub fn set_memory_usage(&mut self, usage: u64) {
        self.memory_usage = usage;
    }

    /// Sets the number of bytes read from disk since last measurement.
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes read
    pub fn set_disk_read_bytes(&mut self, bytes: u64) {
        self.disk_read_bytes = bytes;
    }

    /// Sets the number of bytes written to disk since last measurement.
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes written
    pub fn set_disk_write_bytes(&mut self, bytes: u64) {
        self.disk_write_bytes = bytes;
    }

    /// Sets the total number of bytes read from disk since process start.
    ///
    /// # Arguments
    /// * `bytes` - Total number of bytes read
    pub fn set_total_disk_read_bytes(&mut self, bytes: u64) {
        self.total_disk_read_bytes = bytes;
    }

    /// Sets the total number of bytes written to disk since process start.
    ///
    /// # Arguments
    /// * `bytes` - Total number of bytes written
    pub fn set_total_disk_write_bytes(&mut self, bytes: u64) {
        self.total_disk_write_bytes = bytes;
    }

    /// Gets the current CPU usage percentage.
    pub fn get_cpu_usage(&self) -> f32 {
        self.cpu_usage
    }

    /// Gets the current memory usage in bytes.
    pub fn get_memory_usage(&self) -> u64 {
        self.memory_usage
    }

    /// Gets the number of bytes read from disk since last measurement.
    pub fn get_disk_read_bytes(&self) -> u64 {
        self.disk_read_bytes
    }

    /// Gets the number of bytes written to disk since last measurement.
    pub fn get_disk_write_bytes(&self) -> u64 {
        self.disk_write_bytes
    }

    /// Gets the total number of bytes read from disk since process start.
    pub fn get_total_disk_read_bytes(&self) -> u64 {
        self.total_disk_read_bytes
    }

    /// Gets the total number of bytes written to disk since process start.
    pub fn get_total_disk_write_bytes(&self) -> u64 {
        self.total_disk_write_bytes
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for MicroVmMetrics {
    fn default() -> Self {
        Self::new()
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
            None,
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
        assert_eq!(state.get_group_ip(), loaded_state.get_group_ip());

        Ok(())
    }
}

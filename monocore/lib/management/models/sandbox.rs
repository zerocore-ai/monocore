use chrono::{DateTime, Utc};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A sandbox is an active virtual machine that is managed by Monocore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sandbox {
    /// The name of the sandbox.
    pub name: String,

    /// The Monocore configuration filename that defines the sandbox.
    pub config_file: String,

    /// The last modified date and time of the Monocore configuration file.
    pub config_last_modified: DateTime<Utc>,

    /// The status of the sandbox.
    pub status: String,

    /// The PID of the supervisor process for the sandbox.
    pub supervisor_pid: u32,

    /// The PID of the microVM process for the sandbox.
    pub microvm_pid: u32,

    /// The paths to the root filesystems for the sandbox.
    pub rootfs_paths: String,

    /// The ID of the group that the sandbox belongs to.
    pub group_id: Option<u32>,

    /// The IP address of the group that the sandbox belongs to.
    pub group_ip: Option<String>,
}

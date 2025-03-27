//! Database models for Monocore.

use chrono::{DateTime, Utc};

//--------------------------------------------------------------------------------------------------
// Types: Sandbox
//--------------------------------------------------------------------------------------------------

/// A sandbox is an active virtual machine that is managed by Monocore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sandbox {
    /// The unique identifier for the sandbox.
    pub id: i64,

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

    /// When the sandbox was created
    pub created_at: DateTime<Utc>,

    /// When the sandbox was last modified
    pub modified_at: DateTime<Utc>,
}

//--------------------------------------------------------------------------------------------------
// Types: OCI
//--------------------------------------------------------------------------------------------------

/// Represents an OCI container image in the database
#[derive(Debug, Clone)]
pub struct Image {
    /// Unique identifier for the image
    pub id: i64,

    /// Reference string for the image (e.g. "library/ubuntu:latest")
    pub reference: String,

    /// Size of the image in bytes
    pub size_bytes: i64,

    /// When the image was last used
    pub last_used_at: Option<DateTime<Utc>>,

    /// When the image was created
    pub created_at: DateTime<Utc>,

    /// When the image was last modified
    pub modified_at: DateTime<Utc>,
}

/// Represents an OCI image index in the database
#[derive(Debug, Clone)]
pub struct Index {
    /// Unique identifier for the index
    pub id: i64,

    /// ID of the image this index belongs to
    pub image_id: i64,

    /// Schema version for the index
    pub schema_version: i64,

    /// Media type of the index
    pub media_type: String,

    /// Operating system platform
    pub platform_os: Option<String>,

    /// Architecture platform
    pub platform_arch: Option<String>,

    /// Platform variant
    pub platform_variant: Option<String>,

    /// JSON string containing annotations
    pub annotations_json: Option<String>,

    /// When the index was created
    pub created_at: DateTime<Utc>,

    /// When the index was last modified
    pub modified_at: DateTime<Utc>,
}

/// Represents an OCI image manifest in the database
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Unique identifier for the manifest
    pub id: i64,

    /// Optional ID of the index this manifest belongs to
    pub index_id: Option<i64>,

    /// ID of the image this manifest belongs to
    pub image_id: i64,

    /// Schema version for the manifest
    pub schema_version: i64,

    /// Media type of the manifest
    pub media_type: String,

    /// JSON string containing annotations
    pub annotations_json: Option<String>,

    /// When the manifest was created
    pub created_at: DateTime<Utc>,

    /// When the manifest was last modified
    pub modified_at: DateTime<Utc>,
}

/// Represents an OCI image configuration in the database
#[derive(Debug, Clone)]
pub struct Config {
    /// Unique identifier for the config
    pub id: i64,

    /// ID of the manifest this config belongs to
    pub manifest_id: i64,

    /// Media type of the config
    pub media_type: String,

    /// When the image was created
    pub created: Option<DateTime<Utc>>,

    /// Architecture of the image
    pub architecture: String,

    /// Operating system of the image
    pub os: String,

    /// Operating system variant
    pub os_variant: Option<String>,

    /// JSON string containing environment variables
    pub config_env_json: Option<String>,

    /// JSON string containing command
    pub config_cmd_json: Option<String>,

    /// Working directory
    pub config_working_dir: Option<String>,

    /// JSON string containing entrypoint
    pub config_entrypoint_json: Option<String>,

    /// JSON string containing volumes
    pub config_volumes_json: Option<String>,

    /// JSON string containing exposed ports
    pub config_exposed_ports_json: Option<String>,

    /// User to run as
    pub config_user: Option<String>,

    /// Type of root filesystem
    pub rootfs_type: String,

    /// JSON string containing rootfs diff IDs
    pub rootfs_diff_ids_json: Option<String>,

    /// JSON string containing history
    pub history_json: Option<String>,

    /// When the config was created
    pub created_at: DateTime<Utc>,

    /// When the config was last modified
    pub modified_at: DateTime<Utc>,
}

/// Represents an OCI image layer in the database
#[derive(Debug, Clone)]
pub struct Layer {
    /// Unique identifier for the layer
    pub id: i64,

    /// ID of the manifest this layer belongs to
    pub manifest_id: i64,

    /// Media type of the layer
    pub media_type: String,

    /// Digest (hash) of the compressed layer
    pub digest: String,

    /// Diff ID (hash) of the uncompressed layer
    pub diff_id: String,

    /// Size of the layer in bytes
    pub size_bytes: i64,

    /// When the layer was created
    pub created_at: DateTime<Utc>,

    /// When the layer was last modified
    pub modified_at: DateTime<Utc>,
}

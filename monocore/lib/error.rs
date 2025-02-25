use ipldstore::{ipld, StoreError};
use monofs::FsError;
use monoutils::MonoutilsError;
use nix::errno::Errno;
use sqlx::migrate::MigrateError;
use std::{
    error::Error,
    fmt::{self, Display},
    path::{PathBuf, StripPrefixError},
    time::SystemTimeError,
};
use thiserror::Error;

use crate::oci::DockerRegistryResponseError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The result of a monocore-related operation.
pub type MonocoreResult<T> = Result<T, MonocoreError>;

/// An error that occurred during a file system operation.
#[derive(pretty_error_debug::Debug, Error)]
pub enum MonocoreError {
    /// An I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// An error that can represent any error.
    #[error(transparent)]
    Custom(#[from] AnyError),

    /// An error that occurred during an OCI distribution operation.
    #[error("oci distribution error: {0}")]
    OciDistribution(#[from] anyhow::Error),

    /// An error that occurred during an HTTP request.
    #[error("http request error: {0}")]
    HttpRequest(#[from] reqwest::Error),

    /// An error that occurred during an HTTP middleware operation.
    #[error("http middleware error: {0}")]
    HttpMiddleware(#[from] reqwest_middleware::Error),

    /// An error that occurred during a database operation.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// An error that occurred when a manifest was not found.
    #[error("manifest not found")]
    ManifestNotFound,

    /// An error that occurred when a join handle returned an error.
    #[error("join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    /// An error that occurred when an unsupported image hash algorithm was used.
    #[error("unsupported image hash algorithm: {0}")]
    UnsupportedImageHashAlgorithm(String),

    /// An error that occurred when an image layer download failed.
    #[error("image layer download failed: {0}")]
    ImageLayerDownloadFailed(String),

    /// An error that occurred when an invalid path pair was used.
    #[error("invalid path pair: {0}")]
    InvalidPathPair(String),

    /// An error that occurred when an invalid port pair was used.
    #[error("invalid port pair: {0}")]
    InvalidPortPair(String),

    /// An error that occurred when an invalid environment variable pair was used.
    #[error("invalid environment variable pair: {0}")]
    InvalidEnvPair(String),

    /// An error that occurred when an invalid MicroVm configuration was used.
    #[error("invalid MicroVm configuration: {0}")]
    InvalidMicroVMConfig(InvalidMicroVMConfigError),

    /// An error that occurred when an invalid resource limit format was used.
    #[error("invalid resource limit format: {0}")]
    InvalidRLimitFormat(String),

    /// An error that occurred when an invalid resource limit value was used.
    #[error("invalid resource limit value: {0}")]
    InvalidRLimitValue(String),

    /// An error that occurred when an invalid resource limit resource was used.
    #[error("invalid resource limit resource: {0}")]
    InvalidRLimitResource(String),

    /// An error that occurred when a Serde JSON error occurred.
    #[error("serde json error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    /// An error that occurred when a Serde YAML error occurred.
    #[error("serde yaml error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),

    /// An error that occurred when a TOML error occurred.
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),

    /// An error that occurred when a configuration validation error occurred.
    #[error("configuration validation error: {0}")]
    ConfigValidation(String),

    /// An error that occurred when a configuration validation error occurred.
    #[error("configuration validation errors: {0:?}")]
    ConfigValidationErrors(Vec<String>),

    /// An error that occurs when trying to access group resources for a service that has no group
    #[error("service '{0}' belongs to no group")]
    ServiceBelongsToNoGroup(String),

    /// An error that occurs when trying to access group resources for a service that belongs to a
    /// different group.
    #[error("service '{0}' belongs to wrong group: '{1}'")]
    ServiceBelongsToWrongGroup(String, String),

    /// An error that occurred when failed to get shutdown eventfd
    #[error("failed to get shutdown eventfd: {0}")]
    FailedToGetShutdownEventFd(i32),

    /// An error that occurred when failed to write to shutdown eventfd
    #[error("failed to write to shutdown eventfd: {0}")]
    FailedToShutdown(String),

    /// An error that occurred when failed to start VM
    #[error("failed to start VM: {0}")]
    FailedToStartVM(i32),

    /// An error that occurred when a path does not exist
    #[error("path does not exist: {0}")]
    PathNotFound(String),

    /// An error that occurred when a rootfs path does not exist
    #[error("rootfs path does not exist: {0}")]
    RootFsPathNotFound(String),

    /// An error that occurred when the supervisor binary was not found
    #[error("supervisor binary not found: {0}")]
    SupervisorBinaryNotFound(String),

    /// An error that occurred when failed to start VM
    #[error("failed to start VM: {0}")]
    StartVmFailed(i32),

    /// An error that occurred when waiting for a process to exit
    #[error("process wait error: {0}")]
    ProcessWaitError(String),

    /// An error that occurred when the supervisor task failed
    #[error("supervisor error: {0}")]
    SupervisorError(String),

    /// An error that occurred when failed to kill process
    #[error("failed to kill process: {0}")]
    ProcessKillError(String),

    /// An error that occurred when merging configurations
    #[error("configuration merge error: {0}")]
    ConfigMerge(String),

    /// An error that occurred when no more IP addresses are available for assignment
    #[error("no available IP addresses in the pool")]
    NoAvailableIPs,

    /// An error that occurred during a walkdir operation
    #[error("walkdir error: {0}")]
    WalkDir(#[from] walkdir::Error),

    /// An error that occurred when stripping a path prefix
    #[error("strip prefix error: {0}")]
    StripPrefix(#[from] StripPrefixError),

    /// An error that occurred during a system call
    #[error("system call error: {0}")]
    SystemCall(#[from] Errno),

    /// An error that occurred when converting system time
    #[error("system time error: {0}")]
    SystemTime(#[from] SystemTimeError),

    /// An error that occurred during layer extraction.
    /// This typically happens when the join handle for the blocking task fails.
    #[error("layer extraction error: {0}")]
    LayerExtraction(String),

    /// An error that occurred during layer handling operations like opening files or unpacking archives.
    /// Contains both the underlying IO error and the path to the layer being processed.
    #[error("layer handling error: {source}")]
    LayerHandling {
        /// The underlying IO error that occurred
        source: std::io::Error,
        /// The path to the layer being processed when the error occurred
        layer: String,
    },

    /// An error that occurred when a configuration file was not found
    #[error("configuration file not found: {0}")]
    ConfigNotFound(String),

    /// Error when a service's rootfs directory is not found
    #[error("Service rootfs not found: {0}")]
    RootfsNotFound(String),

    /// An error that occurred when parsing an image reference
    #[error("invalid image reference: {0}")]
    ImageReferenceError(String),

    /// An error that occurred when trying to remove running services
    #[error("Cannot remove running services: {0}")]
    ServiceStillRunning(String),

    /// An error that occurred when invalid command line arguments were provided
    #[error("{0}")]
    InvalidArgument(String),

    /// An error that occurred when validating paths
    #[error("path validation error: {0}")]
    PathValidation(String),

    /// An error that occurred when the monocore config file was not found
    #[error("monocore config file not found at: {0}")]
    MonocoreConfigNotFound(String),

    /// An error that occurred when failed to parse configuration file
    #[error("failed to parse configuration file: {0}")]
    ConfigParseError(String),

    /// An error that occurred when a log file was not found
    #[error("log not found: {0}")]
    LogNotFound(String),

    /// An error that occurred when a pager error occurred
    #[error("pager error: {0}")]
    PagerError(String),

    /// An error from monoutils
    #[error("monoutils error: {0}")]
    MonoutilsError(#[from] MonoutilsError),

    /// An error that occurred when a store error occurred
    #[error("store error: {0}")]
    StoreError(#[from] StoreError),

    /// An error that occurred when a file system error occurred
    #[error("file system error: {0}")]
    FileSystemError(#[from] FsError),
    /// An error that occurred when a migration error occurred
    #[error("migration error: {0}")]
    MigrationError(#[from] MigrateError),

    /// An error that occurred when a Docker registry response error occurred
    #[error("docker registry response error: {0}")]
    DockerRegistryResponseError(#[from] DockerRegistryResponseError),

    /// An error that occurred when parsing an image reference selector with an invalid format
    #[error("invalid image reference    selector format: {0}")]
    InvalidReferenceSelectorFormat(String),

    /// An error that occurred when parsing an invalid digest in an image reference selector
    #[error("invalid image reference selector digest: {0}")]
    InvalidReferenceSelectorDigest(String),

    /// An error that occurred when a feature is not yet implemented
    #[error("feature not yet implemented: {0}")]
    NotImplemented(String),

    /// An error that occurred when a CID error occurred
    #[error("CID error: {0}")]
    CidError(#[from] ipld::cid::Error),

    /// An error that occurred when a sandbox was not found in the configuration
    #[error("sandbox not found in configuration: '{0}' at '{1}'")]
    SandboxNotFoundInConfig(String, PathBuf),

    /// An error that occurs when an invalid log level is used.
    #[error("invalid log level: {0}")]
    InvalidLogLevel(u8),
}

/// An error that occurred when an invalid MicroVm configuration was used.
#[derive(Debug, Error)]
pub enum InvalidMicroVMConfigError {
    /// The root path does not exist.
    #[error("root path does not exist: {0}")]
    RootPathDoesNotExist(String),

    /// A host path that should be mounted does not exist.
    #[error("host path does not exist: {0}")]
    HostPathDoesNotExist(String),

    /// The number of vCPUs is zero.
    #[error("number of vCPUs is zero")]
    NumVCPUsIsZero,

    /// The amount of RAM is zero.
    #[error("amount of RAM is zero")]
    RamIsZero,

    /// The command line contains invalid characters. Only printable ASCII characters (space through tilde) are allowed.
    #[error("command line contains invalid characters (only ASCII characters between space and tilde are allowed): {0}")]
    InvalidCommandLineString(String),

    /// An error that occurs when conflicting guest paths are detected.
    #[error("Conflicting guest paths: '{0}' and '{1}' overlap")]
    ConflictingGuestPaths(String, String),
}

/// An error that can represent any error.
#[derive(Debug)]
pub struct AnyError {
    error: anyhow::Error,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MonocoreError {
    /// Creates a new `Err` result.
    pub fn custom(error: impl Into<anyhow::Error>) -> MonocoreError {
        MonocoreError::Custom(AnyError {
            error: error.into(),
        })
    }
}

impl AnyError {
    /// Downcasts the error to a `T`.
    pub fn downcast<T>(&self) -> Option<&T>
    where
        T: Display + fmt::Debug + Send + Sync + 'static,
    {
        self.error.downcast_ref::<T>()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Creates an `Ok` `MonocoreResult`.
#[allow(non_snake_case)]
pub fn Ok<T>(value: T) -> MonocoreResult<T> {
    Result::Ok(value)
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl PartialEq for AnyError {
    fn eq(&self, other: &Self) -> bool {
        self.error.to_string() == other.error.to_string()
    }
}

impl Display for AnyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl Error for AnyError {}

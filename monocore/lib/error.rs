use std::{
    error::Error,
    fmt::{self, Display},
};
use thiserror::Error;

use crate::oci::distribution::DockerRegistryResponseError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The result of a monocore-related operation.
pub type MonocoreResult<T> = Result<T, MonocoreError>;

/// An error that occurred during a file system operation.
#[derive(Debug, Error)]
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

    /// An error that occurred during a Docker registry operation.
    #[error("docker registry error: {0}")]
    DockerRegistry(#[from] DockerRegistryResponseError),

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

    /// An error that occurred when a configuration validation error occurred.
    #[error("configuration validation error: {0}")]
    ConfigValidation(String),

    /// An error that occurs when trying to access group resources for a service that has no group
    #[error("service '{0}' belongs to no group")]
    ServiceBelongsToNoGroup(String),

    /// An error that occurs when trying to access group resources for a service that belongs to a
    /// different group.
    #[error("service '{0}' belongs to wrong group: '{1}'")]
    ServiceBelongsToWrongGroup(String, String),

    /// An error that occurred when a channel send operation failed.
    #[error("channel send error: {0}")]
    ChannelSendError(#[from] tokio::sync::mpsc::error::SendError<()>),

    /// An error that occurred when failed to get shutdown eventfd
    #[error("failed to get shutdown eventfd: {0}")]
    FailedToGetShutdownEventFd(i32),

    /// An error that occurred when failed to write to shutdown eventfd
    #[error("failed to write to shutdown eventfd: {0}")]
    FailedToShutdown(String),

    /// An error that occurred when failed to start VM
    #[error("failed to start VM: {0}")]
    FailedToStartVM(i32),

    /// An error that occurred when invalid supervisor arguments were provided.
    #[error("invalid supervisor arguments: {0}")]
    InvalidSupervisorArgs(String),

    /// An error that occurred when a service was not found
    #[error("service not found: {0}")]
    ServiceNotFound(String),

    /// An error that occurred when a process ID could not be obtained
    #[error("failed to get process ID for service: {0}")]
    ProcessIdNotFound(String),

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
    #[error("Failed to kill process: {0}")]
    ProcessKillError(String),

    /// An error that occurred when merging configurations
    #[error("configuration merge error: {0}")]
    ConfigMerge(String),
}

/// An error that occurred when an invalid MicroVm configuration was used.
#[derive(Debug, Error)]
pub enum InvalidMicroVMConfigError {
    /// The root path does not exist.
    #[error("root path does not exist: {0}")]
    RootPathDoesNotExist(String),

    /// The number of vCPUs is zero.
    #[error("number of vCPUs is zero")]
    NumVCPUsIsZero,

    /// The amount of RAM is zero.
    #[error("amount of RAM is zero")]
    RamIsZero,
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

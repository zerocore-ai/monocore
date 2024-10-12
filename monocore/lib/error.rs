use std::{error::Error, fmt::Display};
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl Error for AnyError {}

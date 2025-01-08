use std::{
    error::Error,
    fmt::{self, Display},
};

use monoutils_store::ipld::cid::Cid;
use thiserror::Error;

use crate::filesystem::Utf8UnixPathSegment;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The result of a file system operation.
pub type FsResult<T> = Result<T, FsError>;

/// An error that occurred during a file system operation.
#[derive(Debug, Error)]
pub enum FsError {
    /// Infallible error.
    #[error("Infallible error")]
    Infallible(#[from] core::convert::Infallible),

    /// Not a file.
    #[error("Not a file: {0:?}")]
    NotAFile(String),

    /// Not a directory.
    #[error("Not a directory: {0:?}")]
    NotADirectory(String),

    /// Not a softlink.
    #[error("Not a softlink: {0:?}")]
    NotASoftLink(String),

    /// Path not found.
    #[error("Path not found: {0}")]
    PathNotFound(String),

    /// Custom error.
    #[error("Custom error: {0}")]
    Custom(#[from] AnyError),

    // /// DID related error.
    // #[error("DID error: {0}")]
    // Did(#[from] monoutils_did_wk::DidError),
    /// IPLD Store error.
    #[error("IPLD Store error: {0}")]
    IpldStore(#[from] monoutils_store::StoreError),

    /// Invalid deserialized OpenFlag value
    #[error("Invalid OpenFlag value: {0}")]
    InvalidOpenFlag(u8),

    /// Invalid deserialized EntityFlag value
    #[error("Invalid EntityFlag value: {0}")]
    InvalidEntityFlag(u8),

    /// Invalid deserialized PathFlag value
    #[error("Invalid PathFlag value: {0}")]
    InvalidPathFlag(u8),

    /// Invalid path component
    #[error("Invalid path component: {0}")]
    InvalidPathComponent(String),

    /// Invalid search path with root.
    #[error("Invalid search path: {0}")]
    InvalidSearchPath(String),

    /// SoftLink not supported yet.
    #[error("SoftLink not supported yet: path: {0:?}")]
    SoftLinkNotSupportedYet(Vec<Utf8UnixPathSegment>),

    /// Invalid search path empty.
    #[error("Invalid search path empty")]
    InvalidSearchPathEmpty,

    /// Unable to load entity.
    #[error("Unable to load entity: {0}")]
    UnableToLoadEntity(Cid),

    /// CID error.
    #[error("CID error: {0}")]
    CidError(#[from] monoutils_store::ipld::cid::Error),

    /// Path has root.
    #[error("Path has root: {0}")]
    PathHasRoot(String),

    /// Source is not a directory.
    #[error("Source is not a directory: {0}")]
    SourceIsNotADir(String),

    /// Target is not a directory.
    #[error("Target is not a directory: {0}")]
    TargetIsNotADir(String),

    /// Path already exists.
    #[error("Path already exists: {0}")]
    PathExists(String),

    /// Path is empty.
    #[error("Path is empty")]
    PathIsEmpty,

    /// Maximum follow depth reached.
    #[error("Maximum follow depth reached")]
    MaxFollowDepthReached,

    /// Broken softlink.
    #[error("Broken softlink: {0}")]
    BrokenSoftLink(Cid),
}

/// An error that can represent any error.
#[derive(Debug)]
pub struct AnyError {
    error: anyhow::Error,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl FsError {
    /// Creates a new `Err` result.
    pub fn custom(error: impl Into<anyhow::Error>) -> FsError {
        FsError::Custom(AnyError {
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

/// Creates an `Ok` `FsResult`.
#[allow(non_snake_case)]
pub fn Ok<T>(value: T) -> FsResult<T> {
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

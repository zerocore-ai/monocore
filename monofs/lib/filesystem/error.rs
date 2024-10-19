use std::{error::Error, fmt::Display};

use monoutils_store::ipld::cid::Cid;
use thiserror::Error;
use typed_path::Utf8UnixPathBuf;

use crate::dir::Utf8UnixPathSegment;

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

    /// Not found.
    #[error("Not found: {0:?}")]
    NotFound(Utf8UnixPathBuf),

    // /// UCAN error.
    // #[error("UCAN error: {0}")]
    // Ucan(#[from] monoutils_ucan::UcanError),
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
}

// /// Permission error.
// #[derive(Debug, Error)]
// pub enum PermissionError {
//     /// Child descriptor has higher permission than parent.
//     #[error("Child descriptor has higher permission than parent: path: {0}, parent(descriptor_flags: {1:?}) child (descriptor_flags: {2:?}, open_flags: {3:?})")]
//     ChildPermissionEscalation(Path, DescriptorFlags, DescriptorFlags, OpenFlags),
// }

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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl Error for AnyError {}

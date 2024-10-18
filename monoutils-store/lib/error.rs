use std::{error::Error, fmt::Display};

use libipld::Cid;
use thiserror::Error;

use super::Codec;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The result of a file system operation.
pub type StoreResult<T> = Result<T, StoreError>;

/// An error that occurred during a file system operation.
#[derive(Debug, Error, PartialEq)]
pub enum StoreError {
    /// The block was not found.
    #[error("Block not found: {0}")]
    BlockNotFound(Cid),

    /// The node block is too large.
    #[error("Node block too large: {0} > {1}")]
    NodeBlockTooLarge(u64, u64),

    /// The raw block is too large.
    #[error("Raw block too large: {0} > {1}")]
    RawBlockTooLarge(u64, u64),

    /// Codec not supported.
    #[error("Unsupported Codec: {0}")]
    UnsupportedCodec(u64),

    /// Expected block codec does not match the actual codec.
    #[error("Unexpected block codec: expected: {0:?} got: {1:?}")]
    UnexpectedBlockCodec(Codec, Codec),

    /// Custom error.
    #[error("Custom error: {0}")]
    Custom(#[from] AnyError),

    /// Layout error.
    #[error("Layout error: {0}")]
    LayoutError(#[from] LayoutError),
}

/// An error that occurred during a layout operation.
#[derive(Debug, Error, PartialEq)]
pub enum LayoutError {
    /// No leaf block found.
    #[error("No leaf block found")]
    NoLeafBlock,
}

/// An error that can represent any error.
#[derive(Debug)]
pub struct AnyError {
    error: anyhow::Error,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl StoreError {
    /// Creates a new `Err` result.
    pub fn custom(error: impl Into<anyhow::Error>) -> StoreError {
        StoreError::Custom(AnyError {
            error: error.into(),
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Creates an `Ok` `FsResult` d.
#[allow(non_snake_case)]
pub fn Ok<T>(value: T) -> StoreResult<T> {
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

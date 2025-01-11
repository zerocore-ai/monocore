//! Error types for the NFSv4.0 server implementation

use thiserror::Error;

/// Main error type for NFSv4.0 operations
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// XDR encoding/decoding error
    #[error("XDR error: {0}")]
    Xdr(String),

    /// RPC protocol error
    #[error("RPC error: {0}")]
    Rpc(String),

    /// NFS protocol error
    #[error("NFS error: {0}")]
    Nfs(String),

    /// State management error
    #[error("State error: {0}")]
    State(String),
}

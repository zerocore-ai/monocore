//! NFSv4.0 Server Implementation
//! This library provides functionality for creating NFSv4.0 servers.

pub mod error;
pub mod nfs;
pub mod rpc;
pub mod server;
pub mod state;
pub mod xdr;

// Re-export main types
pub use error::Error;
pub use server::NfsServer;

/// Result type for NFSv4.0 operations
pub type Result<T> = std::result::Result<T, Error>;

//! Server module for implementing NFSv3 server functionality.
//!
//! This module provides the core NFSv3 server implementation for monofs, allowing it to serve
//! files and directories over the NFS protocol. The implementation is built on top of the
//! content-addressed storage system provided by monofs.
//!
//! # Main Types
//!
//! - [`MonofsServer`]: The main server type that implements the NFSv3 interface. It handles all
//!   standard NFS operations like file/directory creation, reading, writing, and attribute management.
//!   The server is generic over the storage backend, allowing different storage implementations.
//!
//! - [`MemoryMonofsServer`]: A convenience type alias for a MonofsServer using in-memory storage.
//!   This is primarily useful for testing and development.
//!
//! - [`DiskMonofsServer`]: A convenience type alias for a MonofsServer using filesystem-based storage.
//!   This is the recommended type for production use.
//!
//! # Features
//!
//! - Content-addressed storage for efficient deduplication and versioning
//! - Full NFSv3 protocol support
//! - Thread-safe design for concurrent access
//! - Flexible storage backend through generic implementation
//! - Support for standard Unix file permissions and attributes
//!
//! # Examples
//!
//! ```no_run
//! use monofs::server::MemoryMonofsServer;
//! use monoutils_store::MemoryStore;
//!
//! // Create an in-memory NFS server
//! let server = MemoryMonofsServer::new(MemoryStore::default());
//! ```
//!
//! # Implementation Details
//!
//! The server maintains mappings between NFS file IDs and internal paths, and handles
//! all standard NFS operations including:
//!
//! - File and directory creation/removal
//! - Reading and writing files
//! - Directory listing
//! - File attribute management
//! - Symbolic link operations
//!
//! All operations are implemented in a thread-safe manner, allowing concurrent access
//! from multiple NFS clients.

mod nfs;
mod server;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use nfs::*;
pub use server::*;

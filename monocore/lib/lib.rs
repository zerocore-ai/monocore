//! `monocore` is a secure MicroVM provisioning system for running untrusted code in isolated environments.
//!
//! # Overview
//!
//! monocore provides a robust foundation for running AI workloads in isolated microVMs. It handles:
//! - VM lifecycle management
//! - OCI image distribution
//! - Service orchestration
//! - Network isolation
//! - Resource constraints
//!
//! # Key Features
//!
//! - **Secure Isolation**: True VM-level isolation through microVMs
//! - **Container Experience**: Works with standard OCI/Docker images
//! - **Fast Startup**: Millisecond-level VM provisioning
//! - **Resource Control**: Fine-grained CPU, memory and network limits
//! - **Simple API**: RESTful interface for service management
//!
//! # Architecture
//!
//! monocore consists of several key components:
//!
//! - **VM**: Low-level microVM management using libkrun
//! - **OCI**: Image pulling and layer management
//! - **Orchestration**: Service lifecycle and coordination
//! - **Runtime**: Process supervision and monitoring
//! - **Server**: REST API for remote management
//!
//! # Usage Example
//!
//! ```no_run
//! // TODO
//! ```
//!
//! # Modules
//!
//! - [`cli`] - Command-line interface and argument parsing
//! - [`config`] - Configuration types and validation
//! - [`runtime`] - Process supervision and monitoring
//! - [`utils`] - Common utilities and helpers
//! - [`vm`] - MicroVM configuration and control
//!
//! # Platform Support
//!
//! - Linux: Full support with optional overlayfs (experimental)
//! - macOS: Full support with copy-based layer merging
//! - Windows: Not currently supported
//!
//! # Future Improvements
//!
//! The current experimental overlayfs support will be replaced by monofs,
//! a more robust distributed filesystem designed specifically for container workloads.
//! monofs will provide:
//!
//! - Content-addressed storage
//! - Immutable data structures
//! - Copy-on-write semantics
//! - Proper whiteout handling
//! - Cross-platform support

#![warn(missing_docs)]

mod error;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod cli;
pub mod config;
pub mod management;
pub mod models;
pub mod oci;
pub mod runtime;
pub mod server;
pub mod utils;
pub mod vm;

pub use error::*;

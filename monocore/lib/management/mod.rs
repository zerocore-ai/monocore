//! Management components for the Monocore runtime.
//!
//! This module serves as the central management system for Monocore, providing
//! functionality for managing sandboxes, images, environments, root filesystems,
//! and databases. It coordinates the various components needed for container
//! and sandbox operations.
//!
//! Key components:
//! - `db`: Database management for storing container and sandbox metadata
//! - `image`: Container image handling and registry operations
//! - `menv`: Monocore environment management
//! - `rootfs`: Root filesystem operations for containers
//! - `sandbox`: Sandbox creation and management
//! - `orchestra`: Orchestra management for sandboxes

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod db;
pub mod image;
pub mod menv;
pub mod orchestra;
pub mod rootfs;
pub mod sandbox;

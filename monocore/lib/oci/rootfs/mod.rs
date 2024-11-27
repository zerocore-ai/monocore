//! Container rootfs management and layer merging.
//!
//! This module provides functionality for managing container root filesystems and merging OCI image layers.
//! It supports two layer merging strategies:
//!
//! 1. Copy-based merging (default)
//!    - Works on all platforms
//!    - Handles OCI whiteout files
//!    - Preserves file permissions
//!    - More disk space usage
//!
//! 2. Overlayfs-based merging (Linux-only, experimental)
//!    - Enabled with `overlayfs` feature flag
//!    - More efficient storage usage
//!    - Falls back to copy-merge on failure
//!    - Not recommended for production use yet
//!
//! # Examples
//!
//! ```no_run
//! use std::path::Path;
//! use monocore::oci::rootfs;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Merge layers into rootfs
//! rootfs::merge(
//!     Path::new("/path/to/oci"),
//!     Path::new("/path/to/rootfs"),
//!     "alpine:latest"
//! ).await?;
//!
//! // Copy rootfs to new location
//! rootfs::copy(
//!     Path::new("/path/to/source"),
//!     Path::new("/path/to/dest"),
//!     true  // Process whiteout files
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Feature Flags
//!
//! - `overlayfs` - Enables experimental overlayfs support on Linux
//!   - Not recommended for production use
//!   - Does not support OCI whiteout files
//!   - May have permission issues
//!   - Falls back to copy-based merge on failure
//!   - Will be replaced by monofs in the future for a more robust solution

mod copy;
mod merge;
mod perm_guard;
mod remove;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use copy::*;
pub use merge::*;
pub use perm_guard::*;
pub use remove::*;

//! Container rootfs management and layer merging.
//!
//! Provides functionality for merging OCI image layers into a single rootfs,
//! using overlayfs on Linux and a copy-based fallback on other platforms.

mod merge;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use merge::*;

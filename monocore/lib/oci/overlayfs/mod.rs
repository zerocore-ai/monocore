//! Container rootfs management and layer merging.
//!
//! Provides functionality for merging OCI image layers into a single rootfs,
//! using overlayfs on Linux and a copy-based fallback on other platforms.

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
mod merge;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
pub use merge::*;

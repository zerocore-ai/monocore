//! Container rootfs management and layer merging.
//!
//! Provides functionality for merging OCI image layers into a single rootfs,
//! using overlayfs on Linux and a copy-based fallback on other platforms.

mod copy;
mod merge;
mod perm_guard;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use copy::*;
pub use merge::*;
pub use perm_guard::*;

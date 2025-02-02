//! OCI (Open Container Initiative) module for interacting with container registries.
//!
//! This module provides functionality for:
//! - Pulling container images from OCI-compliant registries
//! - Parsing and validating image references (tags and digests)
//! - Managing image manifests, configurations, and layers

mod implementations;
mod pull;
mod selector;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use implementations::*;
pub use pull::*;
pub use selector::*;

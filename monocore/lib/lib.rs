//! `monocore` is a secure microvm provisioning system.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod error;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod cli;
pub mod config;
pub mod group;
pub mod oci;
pub mod proxy;
pub mod utils;
pub use error::*;

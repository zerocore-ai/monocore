//! `monocore` is a secure microvm provisioning system.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod error;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod config;
pub mod proxy;

pub use error::*;

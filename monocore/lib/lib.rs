//! `monocore` is a secure MicroVm provisioning system.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod error;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod cli;
pub mod config;
pub mod oci;
pub mod orchestration;
pub mod runtime;
pub mod utils;
pub mod vm;

pub use error::*;

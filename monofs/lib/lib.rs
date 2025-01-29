//! `monofs` is an immutable distributed file system.

#![warn(missing_docs)]

mod error;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod cli;
pub mod config;
pub mod filesystem;
pub mod management;
pub mod runtime;
pub mod server;
pub mod store;
pub mod utils;

pub use error::*;

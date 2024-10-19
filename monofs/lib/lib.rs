//! `monofs` is an immutable distributed file system.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod filesystem;
mod stores;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use filesystem::*;
pub use stores::*;

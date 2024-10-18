//! `monofs` is a distributed, decentralized, secure file system.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod filesystem;
mod stores;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use filesystem::*;
pub use stores::*;

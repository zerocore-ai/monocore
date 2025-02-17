//! `virtualfs` is a library for virtual file systems.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod defaults;
mod error;
mod filesystem;
mod implementations;
mod metadata;
mod segment;
mod server;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use defaults::*;
pub use error::*;
pub use filesystem::*;
pub use implementations::*;
pub use metadata::*;
pub use segment::*;
pub use server::*;

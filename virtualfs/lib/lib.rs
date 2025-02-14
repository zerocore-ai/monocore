//! `virtualfs` is a library for virtual file systems.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod error;
mod filesystem;
mod implementations;
mod metadata;
mod segment;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use error::*;
pub use filesystem::*;
pub use implementations::*;
pub use metadata::*;
pub use segment::*;

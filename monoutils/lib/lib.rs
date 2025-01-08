//! `monoutils` is a library containing general utilities for the monocore project.

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod error;
mod path;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use error::*;
pub use path::*;

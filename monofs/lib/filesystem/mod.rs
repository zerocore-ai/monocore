//! Filesystem implementation.

mod entity;
mod error;
mod kind;
mod link;
mod metadata;
mod pathdirs;
mod traits;
//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub mod dir;
pub mod file;
pub mod symlink;

pub use error::*;
pub use link::*;
pub use metadata::*;
pub use pathdirs::*;
pub use traits::*;

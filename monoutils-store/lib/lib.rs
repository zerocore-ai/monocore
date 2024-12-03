//! `monoutils-store` is a library for working with IPLD content-addressed stores (CAS).

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod chunker;
mod error;
mod implementations;
mod layout;
mod merkle;
mod references;
mod seekable;
mod storable;
mod store;
pub(crate) mod utils;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use chunker::*;
pub use error::*;
pub use implementations::*;
pub use layout::*;
pub use merkle::*;
pub use references::*;
pub use seekable::*;
pub use storable::*;
pub use store::*;

//--------------------------------------------------------------------------------------------------
// Re-Exports
//--------------------------------------------------------------------------------------------------

/// Re-exports of the `libipld` crate.
pub mod ipld {
    pub use libipld::{cid, codec, multihash};
}

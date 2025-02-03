//! `monoutils-store` is a library for working with IPLD content-addressed stores (CAS).

#![warn(missing_docs)]
#![allow(clippy::module_inception)]

mod chunker;
mod constants;
mod error;
mod implementations;
mod layout;
mod merkle;
mod references;
mod storable;
mod store;
pub mod utils;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use chunker::*;
pub use constants::*;
pub use error::*;
pub use implementations::*;
pub use layout::*;
pub use merkle::*;
pub use references::*;
pub use storable::*;
pub use store::*;
pub use utils::*;

//--------------------------------------------------------------------------------------------------
// Re-Exports
//--------------------------------------------------------------------------------------------------

/// Re-exports of the `ipld-core` crate.
pub mod ipld {
    pub use ipld_core::*;
}

/// Re-exports of the `multihash` crate.
pub mod multihash {
    pub use multihash::*;
}

/// Re-exports of the `multihash-codetable` crate.
pub mod codetable {
    pub use multihash_codetable::*;
}

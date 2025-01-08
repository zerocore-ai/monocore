use std::future::Future;

use typed_path::{Utf8UnixPath, Utf8UnixPathBuf};

use crate::filesystem::FsResult;

use super::Link;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Utf8UnixPath`] and some lazily loaded value.
pub type PathLink<V> = Link<Utf8UnixPath, V>;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for types that can be stored to a [`Utf8UnixPath`].
pub trait PathStorable: Sized {
    /// Represents the base object that the path represents.
    type Base;

    /// Stores the value to a [`Utf8UnixPath`].
    fn store(&self) -> impl Future<Output = FsResult<Utf8UnixPathBuf>>;

    /// Loads the value from a [`Utf8UnixPath`].
    fn load(path: &Utf8UnixPath, base: Self::Base) -> impl Future<Output = FsResult<Self>>;
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

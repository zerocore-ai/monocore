mod attributes;
mod entity;

use std::{
    fmt::{self, Display},
    str::FromStr,
};

use async_recursion::async_recursion;

use monoutils_store::{ipld::cid::Cid, IpldStore, Storable};

use crate::filesystem::{FsError, FsResult};

use super::{Cached, Link};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Cid`] and some lazily loaded value.
pub type CidLink<V> = Link<Cid, V>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<V> CidLink<V> {
    /// Gets the value that this link points to.
    pub fn get_value(&self) -> Option<&V> {
        match self {
            Self::Encoded { cached, .. } => cached.get(),
            Self::Decoded(value) => Some(value),
        }
    }

    /// Gets a mutable reference to the value that this link points to.
    pub fn get_value_mut(&mut self) -> Option<&mut V> {
        match self {
            Self::Encoded { cached, .. } => cached.get_mut(),
            Self::Decoded(value) => Some(value),
        }
    }

    /// Gets the [`Cid`] of the [`Entity`] that this link points to.
    ///
    /// This will not encode the [`Cid`] if it is not already encoded.
    pub fn get_cid(&self) -> Option<&Cid> {
        match self {
            Self::Encoded { identifier, .. } => Some(identifier),
            Self::Decoded(_) => None,
        }
    }

    /// Resolves the [`Entity`]'s [`Cid`].
    ///
    /// This will attempt to encode the [`Entity`] if it is not already encoded.
    #[async_recursion(?Send)]
    pub async fn resolve_cid<S>(&self) -> FsResult<Cid>
    where
        S: IpldStore,
        V: Storable<S>,
    {
        match self {
            Self::Encoded { identifier, .. } => Ok(*identifier),
            Self::Decoded(value) => Ok(value.store().await?),
        }
    }

    /// Resolves the value that this link points to.
    ///
    /// This will attempt to resolve the value from the store if it is not already decoded.
    pub async fn resolve_value<S>(&self, store: S) -> FsResult<&V>
    where
        S: IpldStore + Send + Sync,
        V: Storable<S>,
    {
        match self {
            Self::Encoded { identifier, cached } => cached
                .get_or_try_init(V::load(identifier, store))
                .await
                .map_err(Into::into),
            Self::Decoded(value) => Ok(value),
        }
    }

    /// Resolves the value that this link points to.
    ///
    /// This will attempt to resolve the value from the store if it is not already decoded.
    pub async fn resolve_value_mut<S>(&mut self, store: S) -> FsResult<&mut V>
    where
        S: IpldStore + Send + Sync,
        V: Storable<S>,
    {
        match self {
            Self::Encoded { identifier, cached } => {
                let value = if let Some(value) = cached.take() {
                    value
                } else {
                    V::load(identifier, store).await?
                };

                // MUTATION SAFETY: When a mutable reference is requested, it usually means the
                // CID might become stale after mutating the value.
                *self = Self::Decoded(value);

                Ok(self.get_value_mut().unwrap())
            }
            Self::Decoded(value) => Ok(value),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<V> From<Cid> for CidLink<V> {
    fn from(cid: Cid) -> Self {
        Self::Encoded {
            identifier: cid,
            cached: Cached::new(),
        }
    }
}

impl<T> FromStr for CidLink<T> {
    type Err = FsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cid = Cid::from_str(s)?;
        Ok(Self::from(cid))
    }
}

impl<T> TryFrom<String> for CidLink<T> {
    type Error = FsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cid = Cid::try_from(value)?;
        Ok(Self::from(cid))
    }
}

impl<T> Display for CidLink<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoded { identifier, .. } => write!(f, "{}", identifier),
            Self::Decoded(value) => write!(f, "{}", value),
        }
    }
}

impl<T> fmt::Debug for CidLink<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoded { identifier, cached } => f
                .debug_struct("CidLink")
                .field("identifier", &identifier)
                .field("cached", &cached.get())
                .finish(),
            Self::Decoded(value) => f
                .debug_struct("CidLink")
                .field("identifier", &value)
                .finish(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use attributes::*;
pub use entity::*;

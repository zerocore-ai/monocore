mod attributes;
mod entity;

use std::{
    fmt::{self, Display},
    str::FromStr,
};

use async_once_cell::OnceCell;

use ipldstore::{ipld::cid::Cid, IpldStore, Storable};

use crate::{FsError, FsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A type alias for `OnceCell` holding a lazily initialized value.
pub type Cached<V> = OnceCell<V>;

/// A link representing an association between an identifier and some lazily loaded value or
/// just the value itself.
///
/// ## Implementation Note
///
/// It is advisable that the value type `V` implements cheap clone semantics (e.g., using `Arc`)
/// since several operations on `Link` require cloning the value. Using types with expensive clone
/// operations may impact performance.
pub enum Link<I, V> {
    /// A link that is encoded and needs to be resolved.
    Encoded {
        /// The identifier of the link, e.g. a URI or CID.
        identifier: I,

        /// The cached value associated with the identifier.
        cached: Cached<V>,
    },

    /// A link that is decoded and can be used directly.
    Decoded(V),
}

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

    /// Returns the CID (Content Identifier) associated with this link.
    ///
    /// For encoded links, returns the existing CID immediately.
    /// For decoded values, stores the value and returns its new CID.
    ///
    /// ## Implementation Note
    ///
    /// Returns a boxed future because the recursive storage operation requires
    /// captured values to be `Send`. Since `&self` is not `Send`, we clone the
    /// necessary data. This is fine when the value type `V` has a cheap clone.
    pub fn resolve_cid<S>(&self) -> futures::future::BoxFuture<FsResult<Cid>>
    where
        S: IpldStore + Clone + Send,
        V: Storable<S> + Send + Clone,
    {
        match self {
            Self::Encoded { identifier, .. } => Box::pin(async move { Ok(*identifier) }),
            Self::Decoded(value) => {
                let value = value.clone();
                Box::pin(async move { Ok(value.store().await?) })
            }
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

impl<I, V> Clone for Link<I, V>
where
    I: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Encoded { identifier, .. } => Self::Encoded {
                identifier: identifier.clone(),
                cached: Cached::new(),
            },
            Self::Decoded(value) => Self::Decoded(value.clone()),
        }
    }
}

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
                .debug_struct("CidLink::Encoded")
                .field("identifier", &identifier)
                .field("cached", &cached.get())
                .finish(),
            Self::Decoded(value) => f.debug_tuple("CidLink::Decoded").field(&value).finish(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use attributes::*;
pub use entity::*;

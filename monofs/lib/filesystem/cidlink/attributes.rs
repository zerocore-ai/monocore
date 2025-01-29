use monoutils_store::IpldStore;

use crate::{filesystem::{CidLink, ExtendedAttributes}, FsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Cid`] and a lazily loaded [`ExtendedAttributes`].
pub type AttributesCidLink<S> = CidLink<ExtendedAttributes<S>>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> AttributesCidLink<S>
where
    S: IpldStore,
{
    /// Gets the [`ExtendedAttributes`] that this link points to.
    ///
    /// This will not resolve the [`ExtendedAttributes`] from the store if it is not already fetched
    /// and decoded.
    #[inline]
    pub fn get_attributes(&self) -> Option<&ExtendedAttributes<S>> {
        self.get_value()
    }

    /// Gets a mutable reference to the [`ExtendedAttributes`] that this link points to.
    ///
    /// This will not resolve the [`ExtendedAttributes`] from the store if it is not already fetched
    /// and decoded.
    #[inline]
    pub fn get_attributes_mut(&mut self) -> Option<&mut ExtendedAttributes<S>> {
        self.get_value_mut()
    }

    /// Resolves the [`ExtendedAttributes`] that this link points to.
    ///
    /// This will attempt to resolve the [`ExtendedAttributes`] from the store if it is not already
    /// decoded.
    #[inline]
    pub async fn resolve_attributes(&self, store: S) -> FsResult<&ExtendedAttributes<S>>
    where
        S: Send + Sync,
    {
        self.resolve_value(store).await
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> From<ExtendedAttributes<S>> for AttributesCidLink<S>
where
    S: IpldStore,
{
    fn from(attributes: ExtendedAttributes<S>) -> Self {
        Self::Decoded(attributes)
    }
}

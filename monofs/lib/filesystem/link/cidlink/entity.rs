use monoutils_store::IpldStore;

use crate::filesystem::{
    dir::Dir, entity::Entity, file::File, symcidlink::SymCidLink, CidLink, FsResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Cid`] and a lazily loaded [`Entity`] or just the
/// [`Entity`] itself.
pub type EntityCidLink<S> = CidLink<Entity<S>>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> EntityCidLink<S>
where
    S: IpldStore,
{
    /// Gets the [`Entity`] that this link points to.
    ///
    /// This will not resolve the [`Entity`] from the store if it is not already fetched and
    /// decoded.
    #[inline]
    pub fn get_entity(&self) -> Option<&Entity<S>> {
        self.get_value()
    }

    /// Gets a mutable reference to the [`Entity`] that this link points to.
    ///
    /// This will not resolve the [`Entity`] from the store if it is not already fetched and
    /// decoded.
    #[inline]
    pub fn get_entity_mut(&mut self) -> Option<&mut Entity<S>> {
        self.get_value_mut()
    }

    /// Resolves the [`Entity`] that this link points to.
    ///
    /// This will attempt to resolve the [`Entity`] from the store if it is not already decoded.
    #[inline]
    pub async fn resolve_entity(&self, store: S) -> FsResult<&Entity<S>>
    where
        S: Send + Sync,
    {
        self.resolve_value(store).await
    }

    /// Resolves the [`Entity`] that this link points to.
    ///
    /// This will attempt to resolve the [`Entity`] from the store if it is not already decoded.
    #[inline]
    pub async fn resolve_entity_mut(&mut self, store: S) -> FsResult<&mut Entity<S>>
    where
        S: Send + Sync,
    {
        self.resolve_value_mut(store).await
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> From<Entity<S>> for EntityCidLink<S>
where
    S: IpldStore,
{
    fn from(entity: Entity<S>) -> Self {
        Self::Decoded(entity)
    }
}

impl<S> From<Dir<S>> for EntityCidLink<S>
where
    S: IpldStore,
{
    fn from(dir: Dir<S>) -> Self {
        Self::Decoded(Entity::Dir(dir))
    }
}

impl<S> From<File<S>> for EntityCidLink<S>
where
    S: IpldStore,
{
    fn from(file: File<S>) -> Self {
        Self::Decoded(Entity::File(file))
    }
}

impl<S> From<SymCidLink<S>> for EntityCidLink<S>
where
    S: IpldStore,
{
    fn from(symlink: SymCidLink<S>) -> Self {
        Self::Decoded(Entity::SymCidLink(symlink))
    }
}

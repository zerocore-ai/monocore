use async_once_cell::OnceCell;
use monoutils_store::{IpldStore, Storable};

use crate::{
    dir::Dir, file::File, filesystem::entity::Entity, symlink::Symlink, CidLink, FsResult,
    Resolvable,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Cid`] and a lazily loaded [`Entity`].
pub type EntityCidLink<S> = CidLink<Entity<S>>;

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<'a, S> Resolvable<'a, S> for EntityCidLink<S>
where
    S: IpldStore + Send + Sync + 'a,
{
    type Target = Entity<S>;

    /// Resolves the [`EntityCidLink`] to an [`Entity`].
    async fn resolve(&'a self, store: S) -> FsResult<&'a Self::Target> {
        self.cached
            .get_or_try_init(Entity::load(&self.identifier, store))
            .await
            .map_err(Into::into)
    }

    /// Resolves the [`EntityCidLink`] to a mutable [`Entity`].
    async fn resolve_mut(&'a mut self, store: S) -> FsResult<&'a mut Self::Target> {
        self.cached
            .get_or_try_init(Entity::load(&self.identifier, store))
            .await?;

        Ok(self.cached.get_mut().unwrap())
    }
}

impl<S> EntityCidLink<S>
where
    S: IpldStore,
{
    /// Change the store used to persist the CID link.
    pub fn use_store<T>(self, _: &T) -> EntityCidLink<T>
    where
        T: IpldStore,
    {
        EntityCidLink {
            identifier: self.identifier,
            cached: OnceCell::new(),
        }
    }

    /// Creates a new [`EntityCidLink`] from an [`Entity`].
    pub async fn from_entity(entity: Entity<S>) -> FsResult<Self>
    where
        S: IpldStore + Send + Sync,
    {
        Self::from_value(entity).await
    }

    /// Creates a new [`EntityCidLink`] from a [`Dir`].
    pub async fn from_dir(dir: Dir<S>) -> FsResult<Self>
    where
        S: Send + Sync,
    {
        Self::from_value(Entity::Dir(dir)).await
    }

    /// Creates a new [`EntityCidLink`] from a [`File`].
    pub async fn from_file(file: File<S>) -> FsResult<Self>
    where
        S: Send + Sync,
    {
        Self::from_value(Entity::File(file)).await
    }

    /// Creates a new [`EntityCidLink`] from a [`Symlink`].
    pub async fn from_symlink(symlink: Symlink<S>) -> FsResult<Self>
    where
        S: Send + Sync,
    {
        Self::from_value(Entity::Symlink(symlink)).await
    }
}

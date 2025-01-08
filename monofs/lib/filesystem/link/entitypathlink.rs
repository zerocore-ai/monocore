use std::str::FromStr;

use async_once_cell::OnceCell;
use monoutils_store::IpldStore;
use typed_path::{Utf8UnixPath, Utf8UnixPathBuf};

use crate::filesystem::{Dir, Entity, FsError, FsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A type alias for `OnceCell` holding a lazily initialized value.
pub type CachedEntity<S> = OnceCell<Entity<S>>;

/// A link representing an association between [`Utf8UnixPath`] and some lazily loaded value.
pub struct EntityPathLink<S>
where
    S: IpldStore,
{
    /// The path of the link.
    path: Utf8UnixPathBuf,

    /// The cached entity associated with the path.
    cached: CachedEntity<S>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> EntityPathLink<S>
where
    S: IpldStore,
{
    /// Gets the cached entity if it exists.
    pub fn get_entity(&self) -> Option<&Entity<S>> {
        self.cached.get()
    }

    /// Gets the path that this link points to.
    pub fn get_path(&self) -> &Utf8UnixPath {
        &self.path
    }

    /// Resolves the entity that this link points to.
    ///
    /// This will attempt to resolve the entity from the directory if it is not already cached.
    pub async fn resolve_entity<'a>(&'a self, dir: &'a Dir<S>) -> FsResult<&'a Entity<S>>
    where
        S: Send + Sync,
    {
        self.cached
            .get_or_try_init(async move {
                let entity = dir
                    .find(&self.path)
                    .await?
                    .ok_or_else(|| FsError::PathNotFound(self.path.to_string()))?;
                Ok::<_, FsError>(entity.clone())
            })
            .await
            .map_err(Into::into)
    }

    /// Resolves the entity that this link points to.
    ///
    /// This will attempt to resolve the entity from the directory if it is not already cached.
    pub async fn resolve_entity_mut<'a>(
        &'a mut self,
        dir: &'a mut Dir<S>,
    ) -> FsResult<&'a mut Entity<S>>
    where
        S: Send + Sync,
    {
        // If we have a cached entity, we need to clear it since we're getting a mutable reference
        self.cached.take();

        // Get the entity from the directory
        let entity = dir
            .find_mut(&self.path)
            .await?
            .ok_or_else(|| FsError::PathNotFound(self.path.to_string()))?;

        // Cache the entity and return a mutable reference
        Ok(entity)
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> Clone for EntityPathLink<S>
where
    S: IpldStore,
{
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            cached: CachedEntity::new(),
        }
    }
}

impl<S> From<Utf8UnixPathBuf> for EntityPathLink<S>
where
    S: IpldStore,
{
    fn from(path: Utf8UnixPathBuf) -> Self {
        Self {
            path,
            cached: CachedEntity::new(),
        }
    }
}

impl<S> FromStr for EntityPathLink<S>
where
    S: IpldStore,
{
    type Err = FsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = Utf8UnixPathBuf::from_str(s)
            .map_err(|e| FsError::InvalidPathComponent(e.to_string()))?;
        Ok(Self::from(path))
    }
}

impl<S> TryFrom<String> for EntityPathLink<S>
where
    S: IpldStore,
{
    type Error = FsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

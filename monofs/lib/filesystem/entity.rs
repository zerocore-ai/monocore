use std::fmt::{self, Debug};

use monoutils_store::{ipld::cid::Cid, IpldStore, Storable, StoreError, StoreResult};

use crate::{dir::Dir, file::File, symlink::Symlink, FsError, FsResult, Metadata};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// This is an entity in the file system.
#[derive(Clone)]
pub enum Entity<S>
where
    S: IpldStore,
{
    /// A file.
    File(File<S>),

    /// A directory.
    Dir(Dir<S>),

    /// A symlink.
    Symlink(Symlink<S>),
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> Entity<S>
where
    S: IpldStore,
{
    /// Returns true if the entity is a file.
    pub fn is_file(&self) -> bool {
        matches!(self, Entity::File(_))
    }

    /// Returns true if the entity is a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self, Entity::Dir(_))
    }

    /// Tries to convert the entity to a file.
    pub fn into_file(self) -> FsResult<File<S>> {
        if let Entity::File(file) = self {
            return Ok(file);
        }
        Err(FsError::NotAFile(String::new()))
    }

    /// Tries to convert the entity to a directory.
    pub fn into_dir(self) -> FsResult<Dir<S>> {
        if let Entity::Dir(dir) = self {
            return Ok(dir);
        }

        Err(FsError::NotADirectory(String::new()))
    }

    /// Tries to convert the entity to a symlink.
    pub fn into_symlink(self) -> FsResult<Symlink<S>> {
        if let Entity::Symlink(symlink) = self {
            return Ok(symlink);
        }

        Err(FsError::NotASymlink(String::new()))
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata {
        match self {
            Entity::File(file) => file.get_metadata(),
            Entity::Dir(dir) => dir.get_metadata(),
            Entity::Symlink(symlink) => symlink.get_metadata(),
        }
    }

    /// Change the store used to persist the entity.
    pub fn use_store<T>(self, store: T) -> Entity<T>
    where
        T: IpldStore,
    {
        match self {
            Entity::File(file) => Entity::File(file.use_store(store)),
            Entity::Dir(dir) => Entity::Dir(dir.use_store(store)),
            Entity::Symlink(symlink) => Entity::Symlink(symlink.use_store(store)),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> Storable<S> for Entity<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        match self {
            Entity::File(file) => file.store().await,
            Entity::Dir(dir) => dir.store().await,
            Entity::Symlink(symlink) => symlink.store().await,
        }
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        // The order of the following `if let` statements is important because for some reason
        // Directory entity deserializes successfully into a File entity even though they have
        // different structure. This is likely due to the way `serde_ipld_dagcbor` deserializes
        // the entities.
        if let Ok(symlink) = Symlink::load(cid, store.clone()).await {
            return Ok(Entity::Symlink(symlink));
        }

        if let Ok(dir) = Dir::load(cid, store.clone()).await {
            return Ok(Entity::Dir(dir));
        }

        if let Ok(file) = File::load(cid, store.clone()).await {
            return Ok(Entity::File(file));
        }

        Err(StoreError::custom(FsError::UnableToLoadEntity(*cid)))
    }
}

impl<S> Debug for Entity<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entity::File(file) => f.debug_tuple("File").field(file).finish(),
            Entity::Dir(dir) => f.debug_tuple("Dir").field(dir).finish(),
            Entity::Symlink(symlink) => f.debug_tuple("Symlink").field(symlink).finish(),
        }
    }
}

impl<S> PartialEq for Entity<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Entity::File(file1), Entity::File(file2)) => file1 == file2,
            (Entity::Dir(dir1), Entity::Dir(dir2)) => dir1 == dir2,
            (Entity::Symlink(symlink1), Entity::Symlink(symlink2)) => symlink1 == symlink2,
            _ => false,
        }
    }
}

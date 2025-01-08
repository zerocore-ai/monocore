use std::fmt::{self, Debug};

use monoutils_store::{ipld::cid::Cid, IpldStore, Storable, StoreError, StoreResult};

use crate::filesystem::{Dir, File, FsError, FsResult, Metadata, SoftLink};

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

    /// A softlink.
    SoftLink(SoftLink<S>),
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

    /// Tries to convert the entity to a softlink.
    pub fn into_softlink(self) -> FsResult<SoftLink<S>> {
        if let Entity::SoftLink(softlink) = self {
            return Ok(softlink);
        }

        Err(FsError::NotASoftLink(String::new()))
    }

    /// Returns the metadata for the entity.
    pub fn get_metadata(&self) -> &Metadata<S> {
        match self {
            Entity::File(file) => file.get_metadata(),
            Entity::Dir(dir) => dir.get_metadata(),
            Entity::SoftLink(softlink) => softlink.get_metadata(),
        }
    }

    /// Returns a mutable reference to the metadata for the entity.
    pub fn get_metadata_mut(&mut self) -> &mut Metadata<S> {
        match self {
            Entity::File(file) => file.get_metadata_mut(),
            Entity::Dir(dir) => dir.get_metadata_mut(),
            Entity::SoftLink(softlink) => softlink.get_metadata_mut(),
        }
    }

    /// Returns the CID of the entity when it was initially loaded from the store.
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        match self {
            Entity::File(file) => file.get_initial_load_cid(),
            Entity::Dir(dir) => dir.get_initial_load_cid(),
            Entity::SoftLink(softlink) => softlink.get_initial_load_cid(),
        }
    }

    /// Returns the CID of the previous version of the entity if there is one.
    pub fn get_previous(&self) -> Option<&Cid> {
        match self {
            Entity::File(file) => file.get_previous(),
            Entity::Dir(dir) => dir.get_previous(),
            Entity::SoftLink(softlink) => softlink.get_previous(),
        }
    }

    /// Returns the store used to persist the entity.
    pub fn get_store(&self) -> &S {
        match self {
            Entity::File(file) => file.get_store(),
            Entity::Dir(dir) => dir.get_store(),
            Entity::SoftLink(softlink) => softlink.get_store(),
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
            Entity::SoftLink(softlink) => softlink.store().await,
        }
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        // The order of the following `if let` statements is important because for some reason
        // Directory entity deserializes successfully into a File entity even though they have
        // different structure. This is likely due to the way `serde_ipld_dagcbor` deserializes
        // the entities.
        if let Ok(softlink) = SoftLink::load(cid, store.clone()).await {
            return Ok(Entity::SoftLink(softlink));
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
            Entity::SoftLink(softlink) => f.debug_tuple("SoftLink").field(softlink).finish(),
        }
    }
}

use std::fmt::{self, Debug};

use ipldstore::{ipld::cid::Cid, IpldStore, Storable, StoreError, StoreResult};
use serde::Deserialize;

use crate::{
    filesystem::{self, Dir, File, Metadata, SymCidLink, SymPathLink},
    FsError, FsResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct TypeField {
    r#type: String,
}

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

    /// A symbolic CID link.
    SymCidLink(SymCidLink<S>),

    /// A symbolic path link.
    SymPathLink(SymPathLink<S>),
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

    /// Returns true if the entity is a symbolic CID link.
    pub fn is_symcidlink(&self) -> bool {
        matches!(self, Entity::SymCidLink(_))
    }

    /// Returns true if the entity is a symbolic path link.
    pub fn is_sympathlink(&self) -> bool {
        matches!(self, Entity::SymPathLink(_))
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

    /// Tries to convert the entity to a symbolic CID link.
    pub fn into_symcidlink(self) -> FsResult<SymCidLink<S>> {
        if let Entity::SymCidLink(symlink) = self {
            return Ok(symlink);
        }

        Err(FsError::NotASymCidLink(String::new()))
    }

    /// Tries to convert the entity to a symbolic path link.
    pub fn into_sympathlink(self) -> FsResult<SymPathLink<S>> {
        if let Entity::SymPathLink(symlink) = self {
            return Ok(symlink);
        }

        Err(FsError::NotASymPathLink(String::new()))
    }

    /// Returns the metadata for the entity.
    pub fn get_metadata(&self) -> &Metadata<S> {
        match self {
            Entity::File(file) => file.get_metadata(),
            Entity::Dir(dir) => dir.get_metadata(),
            Entity::SymCidLink(symlink) => symlink.get_metadata(),
            Entity::SymPathLink(symlink) => symlink.get_metadata(),
        }
    }

    /// Returns a mutable reference to the metadata for the entity.
    pub fn get_metadata_mut(&mut self) -> &mut Metadata<S> {
        match self {
            Entity::File(file) => file.get_metadata_mut(),
            Entity::Dir(dir) => dir.get_metadata_mut(),
            Entity::SymCidLink(symlink) => symlink.get_metadata_mut(),
            Entity::SymPathLink(symlink) => symlink.get_metadata_mut(),
        }
    }

    /// Returns the CID of the entity when it was initially loaded from the store.
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        match self {
            Entity::File(file) => file.get_initial_load_cid(),
            Entity::Dir(dir) => dir.get_initial_load_cid(),
            Entity::SymCidLink(symlink) => symlink.get_initial_load_cid(),
            Entity::SymPathLink(symlink) => symlink.get_initial_load_cid(),
        }
    }

    /// Returns the CID of the previous version of the entity if there is one.
    pub fn get_previous(&self) -> Option<&Cid> {
        match self {
            Entity::File(file) => file.get_previous(),
            Entity::Dir(dir) => dir.get_previous(),
            Entity::SymCidLink(symlink) => symlink.get_previous(),
            Entity::SymPathLink(symlink) => symlink.get_previous(),
        }
    }

    /// Returns a reference to the store.
    pub fn get_store(&self) -> &S {
        match self {
            Entity::File(file) => file.get_store(),
            Entity::Dir(dir) => dir.get_store(),
            Entity::SymCidLink(symlink) => symlink.get_store(),
            Entity::SymPathLink(symlink) => symlink.get_store(),
        }
    }

    /// Returns the size of the entity in bytes.
    pub async fn get_size(&self) -> FsResult<u64> {
        match self {
            Entity::File(file) => file.get_size().await,
            _ => Ok(0),
        }
    }

    /// Creates a checkpoint of the current entity state.
    ///
    /// This is equivalent to storing the entity and loading it back,
    /// which is a common pattern when working with versioned entities.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Entity, File};
    /// use ipldstore::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let file = File::with_content(store.clone(), b"Hello, World!".as_slice()).await?;
    /// let mut entity = Entity::File(file);
    ///
    /// // Store and checkpoint the entity
    /// let cid = entity.checkpoint().await?;
    ///
    /// assert_eq!(entity.get_initial_load_cid(), Some(&cid));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn checkpoint(&mut self) -> StoreResult<Cid>
    where
        S: Send + Sync,
    {
        let cid = self.store().await?;
        let store = self.get_store().clone();
        let loaded = Self::load(&cid, store).await?;
        *self = loaded;
        Ok(cid)
    }

    pub(crate) fn set_previous(&mut self, previous: Option<Cid>) {
        match self {
            Entity::File(file) => file.set_previous(previous),
            Entity::Dir(dir) => dir.set_previous(previous),
            Entity::SymCidLink(symlink) => symlink.set_previous(previous),
            Entity::SymPathLink(symlink) => symlink.set_previous(previous),
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
            Entity::SymCidLink(symlink) => symlink.store().await,
            Entity::SymPathLink(symlink) => symlink.store().await,
        }
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        // First, get the raw bytes to check the type field
        let type_field: TypeField = store.get_node(cid).await?;

        match type_field.r#type.as_str() {
            filesystem::DIR_TYPE_TAG => Ok(Entity::Dir(Dir::load(cid, store).await?)),
            filesystem::FILE_TYPE_TAG => Ok(Entity::File(File::load(cid, store).await?)),
            filesystem::SYMCIDLINK_TYPE_TAG => {
                Ok(Entity::SymCidLink(SymCidLink::load(cid, store).await?))
            }
            filesystem::SYMPATHLINK_TYPE_TAG => {
                Ok(Entity::SymPathLink(SymPathLink::load(cid, store).await?))
            }
            _ => Err(StoreError::custom(FsError::UnableToLoadEntity(*cid))),
        }
    }
}

impl<S> Debug for Entity<S>
where
    S: IpldStore + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entity::File(file) => f.debug_tuple("File").field(file).finish(),
            Entity::Dir(dir) => f.debug_tuple("Dir").field(dir).finish(),
            Entity::SymCidLink(symlink) => f.debug_tuple("SymCidLink").field(symlink).finish(),
            Entity::SymPathLink(symlink) => f.debug_tuple("SymPathLink").field(symlink).finish(),
        }
    }
}

impl<S> From<Dir<S>> for Entity<S>
where
    S: IpldStore + Clone,
{
    fn from(dir: Dir<S>) -> Self {
        Entity::Dir(dir)
    }
}

impl<S> From<File<S>> for Entity<S>
where
    S: IpldStore + Clone,
{
    fn from(file: File<S>) -> Self {
        Entity::File(file)
    }
}

impl<S> From<SymCidLink<S>> for Entity<S>
where
    S: IpldStore + Clone,
{
    fn from(symlink: SymCidLink<S>) -> Self {
        Entity::SymCidLink(symlink)
    }
}

impl<S> From<SymPathLink<S>> for Entity<S>
where
    S: IpldStore + Clone,
{
    fn from(symlink: SymPathLink<S>) -> Self {
        Entity::SymPathLink(symlink)
    }
}

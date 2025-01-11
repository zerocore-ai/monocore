mod find;
mod ops;
mod segment;

use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Debug},
    str::FromStr,
    sync::{Arc, OnceLock},
};

use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};

use crate::filesystem::{
    kind::EntityType, Entity, EntityCidLink, File, FsError, FsResult, Link, Metadata,
    MetadataSerializable, SymCidLink,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The type identifier for directories.
pub const DIR_TYPE_TAG: &str = "monofs.dir";

//--------------------------------------------------------------------------------------------------
// Types: Dir
//--------------------------------------------------------------------------------------------------

/// Represents a directory node in the `monofs` _immutable_ file system.
///
/// ## Important
///
/// Entities in `monofs` are designed to be immutable and clone-on-write meaning writes create
/// forks of the entity.
#[derive(Clone)]
pub struct Dir<S>
where
    S: IpldStore,
{
    pub(super) inner: Arc<DirInner<S>>,
}

#[derive(Clone)]
pub(super) struct DirInner<S>
where
    S: IpldStore,
{
    /// The CID of the directory when it is initially loaded from the store.
    ///
    /// It is not initialized if the directory was not loaded from the store.
    initial_load_cid: OnceLock<Cid>,

    /// The CID of the previous version of the directory if there is one.
    previous: Option<Cid>,

    /// Directory metadata.
    metadata: Metadata<S>,

    /// The store used to persist blocks in the directory.
    store: S,

    /// The entries in the directory.
    entries: HashMap<Utf8UnixPathSegment, EntityCidLink<S>>,
}

//--------------------------------------------------------------------------------------------------
// Types: *
//--------------------------------------------------------------------------------------------------

/// A serializable representation of [`Dir`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirSerializable {
    /// The type of the entity.
    pub r#type: String,

    /// The metadata of the directory.
    metadata: MetadataSerializable,

    /// The entries in the directory.
    entries: BTreeMap<String, Cid>,

    /// The CID of the previous version of the directory if there is one.
    previous: Option<Cid>,
}

//--------------------------------------------------------------------------------------------------
// Methods: Dir
//--------------------------------------------------------------------------------------------------

impl<S> Dir<S>
where
    S: IpldStore,
{
    /// Creates a new directory with the given store.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::Dir;
    /// use monoutils_store::MemoryStore;
    ///
    /// let store = MemoryStore::default();
    /// let dir = Dir::new(store);
    ///
    /// assert!(dir.is_empty());
    /// ```
    pub fn new(store: S) -> Self {
        Self {
            inner: Arc::new(DirInner {
                initial_load_cid: OnceLock::new(),
                previous: None,
                metadata: Metadata::new(EntityType::Dir, store.clone()),
                entries: HashMap::new(),
                store,
            }),
        }
    }

    /// Checks if an [`EntityCidLink`] with the given name exists in the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Utf8UnixPathSegment};
    /// use monoutils_store::MemoryStore;
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store);
    ///
    /// let file_name = "example.txt";
    /// let file_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    ///
    /// dir.put_entry(file_name, file_cid.into())?;
    ///
    /// assert!(dir.has_entry(file_name)?);
    /// assert!(!dir.has_entry("nonexistent.txt")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_entry(&self, name: impl AsRef<str>) -> FsResult<bool> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        Ok(self.inner.entries.contains_key(&name))
    }

    /// Returns the CID of the directory when it was initially loaded from the store.
    ///
    /// It returns `None` if the directory was not loaded from the store.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::Dir;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let dir = Dir::new(store.clone());
    ///
    /// // Initially, the CID is not set
    /// assert!(dir.get_initial_load_cid().is_none());
    ///
    /// // Store the directory
    /// let stored_cid = dir.store().await?;
    ///
    /// // Load the directory
    /// let loaded_dir = Dir::load(&stored_cid, store).await?;
    ///
    /// // Now the initial load CID is set
    /// assert_eq!(loaded_dir.get_initial_load_cid(), Some(&stored_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        self.inner.initial_load_cid.get()
    }

    /// Returns the CID of the previous version of the directory if there is one.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::Dir;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Initially, there's no previous version
    /// assert!(dir.get_previous().is_none());
    ///
    /// // Store the directory
    /// let first_cid = dir.store().await?;
    ///
    /// // Load the directory and create a new version
    /// let mut loaded_dir = Dir::load(&first_cid, store.clone()).await?;
    /// loaded_dir.put_entry("new_file", Cid::default().into())?;
    ///
    /// // Store the new version
    /// let second_cid = loaded_dir.store().await?;
    ///
    /// // Load the new version
    /// let new_version = Dir::load(&second_cid, store).await?;
    ///
    /// // Now the previous CID is set
    /// assert_eq!(new_version.get_previous(), Some(&first_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_previous(&self) -> Option<&Cid> {
        self.inner.previous.as_ref()
    }

    /// Adds a [`EntityCidLink`] and its associated name in the directory's entries.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Utf8UnixPathSegment};
    /// use monoutils_store::MemoryStore;
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store);
    ///
    /// let file_name = "example.txt";
    /// let file_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    ///
    /// dir.put_entry(file_name, file_cid.into())?;
    ///
    /// assert!(dir.get_entry(file_name)?.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn put_entry(&mut self, name: impl AsRef<str>, link: EntityCidLink<S>) -> FsResult<()> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        let inner = Arc::make_mut(&mut self.inner);
        inner.entries.insert(name, link);

        Ok(())
    }

    /// Adds an [`Entity`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_entity(&mut self, name: impl AsRef<str>, entity: Entity<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(entity))
    }

    /// Adds a [`Dir`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_dir(&mut self, name: impl AsRef<str>, dir: Dir<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(dir))
    }

    /// Adds a [`File`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_file(&mut self, name: impl AsRef<str>, file: File<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(file))
    }

    /// Adds a [`SymCidLink`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_symcidlink(&mut self, name: impl AsRef<str>, symlink: SymCidLink<S>) -> FsResult<()>
    where
        S: IpldStore,
    {
        self.put_entry(name, EntityCidLink::from(symlink))
    }

    /// Gets the [`EntityCidLink`] with the given name from the directory's entries.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Utf8UnixPathSegment};
    /// use monoutils_store::MemoryStore;
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store);
    ///
    /// let file_name = "example.txt";
    /// let file_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    ///
    /// dir.put_entry(file_name, file_cid.clone().into())?;
    ///
    /// let entry = dir.get_entry(file_name)?.unwrap();
    /// assert_eq!(entry.resolve_cid().await?, file_cid);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn get_entry(&self, name: impl AsRef<str>) -> FsResult<Option<&EntityCidLink<S>>> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        Ok(self.inner.entries.get(&name))
    }

    /// Gets the [`EntityCidLink`] with the given name from the directory's entries.
    #[inline]
    pub fn get_entry_mut(
        &mut self,
        name: impl AsRef<str>,
    ) -> FsResult<Option<&mut EntityCidLink<S>>> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        let inner = Arc::make_mut(&mut self.inner);
        Ok(inner.entries.get_mut(&name))
    }

    /// Gets the [`Entity`] with the associated name from the directory's entries.
    pub async fn get_entity(&self, name: impl AsRef<str>) -> FsResult<Option<&Entity<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entry(name)? {
            Some(link) => {
                let entity = link.resolve_entity(self.inner.store.clone()).await?;
                if entity.get_metadata().get_deleted_at().is_some() {
                    Ok(None)
                } else {
                    Ok(Some(entity))
                }
            }
            None => Ok(None),
        }
    }

    /// Gets the [`Entity`] with the associated name from the directory's entries.
    pub async fn get_entity_mut(
        &mut self,
        name: impl AsRef<str>,
    ) -> FsResult<Option<&mut Entity<S>>>
    where
        S: Send + Sync,
    {
        let store = self.inner.store.clone();
        match self.get_entry_mut(name)? {
            Some(link) => {
                let entity = link.resolve_entity_mut(store).await?;
                if entity.get_metadata().get_deleted_at().is_some() {
                    Ok(None)
                } else {
                    Ok(Some(entity))
                }
            }
            None => Ok(None),
        }
    }

    /// Gets the [`Dir`] with the associated name from the directory's entries.
    pub async fn get_dir(&self, name: impl AsRef<str>) -> FsResult<Option<&Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::Dir(dir)) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`Dir`] with the associated name from the directory's entries.
    pub async fn get_dir_mut(&mut self, name: impl AsRef<str>) -> FsResult<Option<&mut Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity_mut(name).await? {
            Some(Entity::Dir(dir)) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] with the associated name from the directory's entries.
    pub async fn get_file(&self, name: impl AsRef<str>) -> FsResult<Option<&File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::File(file)) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] with the associated name from the directory's entries.
    pub async fn get_file_mut(&mut self, name: impl AsRef<str>) -> FsResult<Option<&mut File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity_mut(name).await? {
            Some(Entity::File(file)) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`SymCidLink`] with the associated name from the directory's entries.
    pub async fn get_symcidlink(&self, name: impl AsRef<str>) -> FsResult<Option<&SymCidLink<S>>>
    where
        S: IpldStore + Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::SymCidLink(symlink)) => Ok(Some(symlink)),
            _ => Ok(None),
        }
    }

    /// Gets the [`SymCidLink`] with the associated name from the directory's entries.
    pub async fn get_symcidlink_mut(
        &mut self,
        name: impl AsRef<str>,
    ) -> FsResult<Option<&mut SymCidLink<S>>>
    where
        S: IpldStore + Send + Sync,
    {
        match self.get_entity_mut(name).await? {
            Some(Entity::SymCidLink(symlink)) => Ok(Some(symlink)),
            _ => Ok(None),
        }
    }

    /// Removes the [`EntityCidLink`] with the given name from the directory's entries.
    pub fn remove_entry(&mut self, name: impl AsRef<str>) -> FsResult<EntityCidLink<S>> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        let inner = Arc::make_mut(&mut self.inner);
        inner
            .entries
            .remove(&name)
            .ok_or(FsError::PathNotFound(name.to_string()))
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata<S> {
        &self.inner.metadata
    }

    /// Returns a mutable reference to the metadata for the directory.
    pub fn get_metadata_mut(&mut self) -> &mut Metadata<S> {
        let inner = Arc::make_mut(&mut self.inner);
        &mut inner.metadata
    }

    /// Returns an iterator over the entries in the directory.
    pub fn get_entries(&self) -> impl Iterator<Item = (&Utf8UnixPathSegment, &EntityCidLink<S>)> {
        self.inner.entries.iter()
    }

    /// Returns the store used to persist the file.
    pub fn get_store(&self) -> &S {
        &self.inner.store
    }

    /// Returns `true` if the directory is empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Utf8UnixPathSegment};
    /// use monoutils_store::MemoryStore;
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store);
    ///
    /// assert!(dir.is_empty());
    ///
    /// let file_name = "example.txt";
    /// let file_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    ///
    /// dir.put_entry(file_name, file_cid.into())?;
    ///
    /// assert!(!dir.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.entries.is_empty()
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub fn from_serializable(
        serializable: DirSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        let entries: HashMap<_, _> = serializable
            .entries
            .into_iter()
            .map(|(segment, cid)| Ok((segment.parse()?, Link::from(cid))))
            .collect::<FsResult<_>>()?;

        Ok(Dir {
            inner: Arc::new(DirInner {
                initial_load_cid: OnceLock::from(load_cid),
                previous: serializable.previous,
                metadata: Metadata::from_serializable(serializable.metadata, store.clone())?,
                store,
                entries,
            }),
        })
    }

    /// Returns a serializable representation of the directory.
    pub async fn get_serializable(&self) -> FsResult<DirSerializable>
    where
        S: Send + Sync,
    {
        let mut entries = BTreeMap::new();
        for (k, v) in self.get_entries() {
            entries.insert(
                k.to_string(),
                v.resolve_cid().await.map_err(StoreError::custom)?,
            );
        }

        let metadata = self.get_metadata().get_serializable().await?;

        Ok(DirSerializable {
            r#type: DIR_TYPE_TAG.to_string(),
            previous: self.inner.initial_load_cid.get().cloned(),
            metadata,
            entries,
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> Storable<S> for Dir<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let serializable = self.get_serializable().await.map_err(StoreError::custom)?;
        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable: DirSerializable = store.get_node(cid).await?;
        Dir::from_serializable(serializable, store, *cid).map_err(StoreError::custom)
    }
}

impl<S> Debug for Dir<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dir")
            .field("metadata", &self.get_metadata())
            .field(
                "entries",
                &self
                    .get_entries()
                    .map(|(_, v)| v.get_cid()) // TODO: Resolve value here.
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl IpldReferences for DirSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(self.entries.values())
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use anyhow::Ok;
    use monoutils_store::MemoryStore;

    use crate::{config::DEFAULT_SYMLINK_DEPTH, filesystem::SyncType};

    use super::*;

    #[tokio::test]
    async fn test_dir_constructor() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let dir = Dir::new(store);

        assert!(dir.inner.entries.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_put_get_entries() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());

        let file1_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
        let file2_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        let file1_name = "file1";
        let file2_name = "file2";

        dir.put_entry(file1_name, file1_cid.into())?;
        dir.put_entry(file2_name, file2_cid.into())?;

        assert_eq!(dir.inner.entries.len(), 2);
        assert_eq!(
            dir.get_entry(&file1_name)?.unwrap().get_cid(),
            Some(&file1_cid)
        );
        assert_eq!(
            dir.get_entry(&file2_name)?.unwrap().get_cid(),
            Some(&file2_cid)
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_stores_loads() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        let file_name = "file1";
        let file_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        dir.put_entry(file_name, file_cid.into())?;

        let cid = dir.store().await?;
        let loaded_dir = Dir::load(&cid, store.clone()).await?;

        // Assert that the metadata is the same
        let dir_metadata = dir.get_metadata();
        let loaded_dir_metadata = loaded_dir.get_metadata();
        assert_eq!(
            dir_metadata.get_entity_type(),
            loaded_dir_metadata.get_entity_type()
        );
        assert_eq!(
            dir_metadata.get_symlink_depth(),
            loaded_dir_metadata.get_symlink_depth()
        );
        assert_eq!(
            dir_metadata.get_sync_type(),
            loaded_dir_metadata.get_sync_type()
        );
        assert_eq!(
            dir_metadata.get_deleted_at(),
            loaded_dir_metadata.get_deleted_at()
        );
        assert_eq!(
            dir_metadata.get_created_at(),
            loaded_dir_metadata.get_created_at()
        );
        assert_eq!(
            dir_metadata.get_modified_at(),
            loaded_dir_metadata.get_modified_at()
        );

        // Assert that the number of entries is the same
        assert_eq!(dir.get_entries().count(), loaded_dir.get_entries().count());

        // Assert that the entry we added exists in the loaded directory
        let loaded_entry = loaded_dir
            .get_entry(&file_name)?
            .expect("Entry should exist");

        assert_eq!(loaded_entry.get_cid(), Some(&file_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_has_entry() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());
        let file_name = "example.txt";
        let file_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        dir.put_entry(file_name, file_cid.into())?;

        assert!(dir.has_entry(file_name)?);
        assert!(!dir.has_entry("nonexistent.txt")?);

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_remove_entry() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());
        let file_name = "example.txt";
        let file_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        dir.put_entry(file_name, file_cid.clone().into())?;
        assert!(dir.has_entry(file_name)?);

        let removed_entry = dir.remove_entry(file_name)?;
        assert_eq!(removed_entry.get_cid(), Some(&file_cid));
        assert!(!dir.has_entry(file_name)?);

        assert!(dir.remove_entry("nonexistent.txt").is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_metadata() -> anyhow::Result<()> {
        let dir = Dir::new(MemoryStore::default());
        let metadata = dir.get_metadata();

        assert_eq!(*metadata.get_entity_type(), EntityType::Dir);
        assert_eq!(*metadata.get_symlink_depth(), DEFAULT_SYMLINK_DEPTH);
        assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
        assert!(metadata.get_deleted_at().is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_is_empty() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());
        assert!(dir.is_empty());

        let file_name = "example.txt";
        let file_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        dir.put_entry(file_name, file_cid.into())?;
        assert!(!dir.is_empty());

        dir.remove_entry(file_name)?;
        assert!(dir.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_entries() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());
        let file1_name = "file1.txt";
        let file2_name = "file2.txt";
        let file1_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
        let file2_cid: Cid =
            "bafkreihwsnuregceqh263vgdaatvch6micl2phrh2tdwkaqsch7jpo5nuu".parse()?;

        dir.put_entry(file1_name, file1_cid.clone().into())?;
        dir.put_entry(file2_name, file2_cid.clone().into())?;

        let entries: Vec<_> = dir.get_entries().collect();
        assert_eq!(entries.len(), 2);

        let entry1 = entries
            .iter()
            .find(|(name, _)| name.as_str() == file1_name)
            .unwrap();
        let entry2 = entries
            .iter()
            .find(|(name, _)| name.as_str() == file2_name)
            .unwrap();

        assert_eq!(entry1.1.get_cid(), Some(&file1_cid));
        assert_eq!(entry2.1.get_cid(), Some(&file2_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_initial_load_cid() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let dir = Dir::new(store.clone());

        // Initially, the CID is not set
        assert!(dir.get_initial_load_cid().is_none());

        // Store the directory
        let stored_cid = dir.store().await?;

        // Load the directory
        let loaded_dir = Dir::load(&stored_cid, store).await?;

        // Now the initial load CID is set
        assert_eq!(loaded_dir.get_initial_load_cid(), Some(&stored_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_previous() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let dir = Dir::new(store.clone());

        // Initially, there's no previous version
        assert!(dir.get_previous().is_none());

        // Store the directory
        let first_cid = dir.store().await?;

        // Load the directory and create a new version
        let mut loaded_dir = Dir::load(&first_cid, store.clone()).await?;
        loaded_dir.put_entry("new_file", Cid::default().into())?;

        // Store the new version
        let second_cid = loaded_dir.store().await?;

        // Load the new version
        let new_version = Dir::load(&second_cid, store).await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(new_version.get_previous(), Some(&first_cid));
        assert_eq!(new_version.get_initial_load_cid(), Some(&second_cid));

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use find::*;
pub use segment::*;

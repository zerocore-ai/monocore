mod find;
mod ops;
mod segment;

use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Debug},
    str::FromStr,
    sync::{Arc, OnceLock},
};

use chrono::Utc;
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
    entries: HashMap<Utf8UnixPathSegment, Entry<S>>,
}

/// Represents an entry in a directory.
#[derive(Clone)]
struct Entry<S>
where
    S: IpldStore,
{
    /// Whether the entry has been deleted.
    pub deleted: bool,

    /// The link to the entity.
    pub link: EntityCidLink<S>,
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
    entries: BTreeMap<String, (bool, Cid)>,

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
    /// let dir = Dir::new(MemoryStore::default());
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

    /// Returns the CID of the directory when it was initially loaded from the store.
    ///
    /// It returns `None` if the directory was not loaded from the store.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::Dir;
    /// use monoutils_store::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Store and checkpoint the directory
    /// let cid = dir.checkpoint().await?;
    ///
    /// assert_eq!(dir.get_initial_load_cid(), Some(&cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        self.inner.initial_load_cid.get()
    }

    /// Creates a checkpoint of the current directory state.
    ///
    /// This is equivalent to storing the directory and loading it back,
    /// which is a common pattern when working with versioned directories.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Add a file and checkpoint
    /// dir.put_adapted_file("test.txt", File::new(store.clone())).await?;
    /// let cid = dir.checkpoint().await?;
    ///
    /// assert_eq!(dir.get_initial_load_cid(), Some(&cid));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn checkpoint(&mut self) -> StoreResult<Cid>
    where
        S: Send + Sync,
    {
        let cid = self.store().await?;
        let store = self.inner.store.clone();
        let loaded = Self::load(&cid, store).await?;
        self.inner = loaded.inner;
        Ok(cid)
    }

    /// Returns the CID of the previous version of the directory if there is one.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Initially no previous version
    /// assert!(dir.get_previous().is_none());
    ///
    /// // Store initial version
    /// let initial_cid = dir.checkpoint().await?;
    ///
    /// // Create a new file and add it
    /// let file = File::new(store.clone());
    /// dir.put_adapted_file("test.txt", file).await?;
    ///
    /// // Store new version
    /// let new_cid = dir.checkpoint().await?;
    ///
    /// // Previous version is set to initial CID
    /// assert_eq!(dir.get_previous(), Some(&initial_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_previous(&self) -> Option<&Cid> {
        self.inner.previous.as_ref()
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

    /// Returns the store used to persist the file.
    pub fn get_store(&self) -> &S {
        &self.inner.store
    }

    /// Returns an iterator over all the entries in the directory.
    ///
    /// This method returns an iterator that yields tuples containing the name and link
    /// for ALL entries in the directory, including ones marked as deleted.
    ///
    /// This differs from [`get_entries`][Self::get_entries] which only returns non-deleted entries.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Add and delete an entry
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    /// dir.remove_entry("test.txt")?;
    ///
    /// assert_eq!(dir.get_all_entries().count(), 1); // Includes deleted entry
    /// assert_eq!(dir.get_entries().count(), 0);     // Excludes deleted entry
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_all_entries(
        &self,
    ) -> impl Iterator<Item = (&Utf8UnixPathSegment, &EntityCidLink<S>)> {
        self.inner.entries.iter().map(|(k, v)| (k, &v.link))
    }

    /// Returns an iterator over the non-deleted entries in the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Add two files
    /// dir.put_adapted_file("a.txt", File::new(store.clone())).await?;
    /// dir.put_adapted_file("b.txt", File::new(store)).await?;
    ///
    /// assert_eq!(dir.get_entries().count(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_entries(&self) -> impl Iterator<Item = (&Utf8UnixPathSegment, &EntityCidLink<S>)> {
        self.inner
            .entries
            .iter()
            .filter(|(_, entry)| !entry.deleted)
            .map(|(k, v)| (k, &v.link))
    }

    /// Returns an iterator over the entry links of entries in the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    /// assert_eq!(dir.get_entry_links().count(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_entry_links(&self) -> impl Iterator<Item = &EntityCidLink<S>> {
        self.inner
            .entries
            .values()
            .filter(|entry| !entry.deleted)
            .map(|entry| &entry.link)
    }

    /// Returns an iterator over the names of entries in the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    /// assert_eq!(dir.get_entry_names().next(), Some(&"test.txt".parse()?));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_entry_names(&self) -> impl Iterator<Item = &Utf8UnixPathSegment> {
        self.inner
            .entries
            .iter()
            .filter(|(_, entry)| !entry.deleted)
            .map(|(k, _)| k)
    }

    /// Marks the entry with the given name as deleted in the directory's entries.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Add and remove a file
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    /// dir.remove_entry("test.txt")?;
    ///
    /// assert!(!dir.has_entry("test.txt")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn remove_entry(&mut self, name: impl AsRef<str>) -> FsResult<&mut EntityCidLink<S>> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        let inner = Arc::make_mut(&mut self.inner);
        let (_, entry) = inner
            .entries
            .iter_mut()
            .find(|(entry_name, _)| *entry_name == &name)
            .ok_or(FsError::PathNotFound(name.to_string()))?;

        entry.deleted = true;
        inner.metadata.set_modified_at(Utc::now());

        Ok(&mut entry.link)
    }

    /// Returns the number of non-deleted entries in the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// assert_eq!(dir.len(), 0);
    ///
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    /// assert_eq!(dir.len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn len(&self) -> usize {
        self.inner
            .entries
            .iter()
            .filter(|(_, entry)| !entry.deleted)
            .count()
    }

    /// Returns `true` if the directory has no entries.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::Dir;
    /// use monoutils_store::MemoryStore;
    ///
    /// let dir = Dir::new(MemoryStore::default());
    /// assert!(dir.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Checks if an entry with the given name exists in the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    ///
    /// assert!(dir.has_entry("test.txt")?);
    /// assert!(!dir.has_entry("missing.txt")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_entry(&self, name: impl AsRef<str>) -> FsResult<bool> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        Ok(self.get_entry_names().any(|n| n == &name))
    }

    /// Gets the entry with the given name from the directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    ///
    /// assert!(dir.get_entry("test.txt")?.is_some());
    /// assert!(dir.get_entry("missing.txt")?.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_entry(&self, name: impl AsRef<str>) -> FsResult<Option<&EntityCidLink<S>>> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        Ok(self
            .get_entries()
            .find(|(n, _)| *n == &name)
            .map(|(_, link)| link))
    }

    /// Gets a mutable reference to the entry with the given name.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// dir.put_adapted_file("test.txt", File::new(store)).await?;
    ///
    /// assert!(dir.get_entry_mut("test.txt")?.is_some());
    /// assert!(dir.get_entry_mut("missing.txt")?.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_entry_mut(
        &mut self,
        name: impl AsRef<str>,
    ) -> FsResult<Option<&mut EntityCidLink<S>>> {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        let inner = Arc::make_mut(&mut self.inner);
        let entry = inner
            .entries
            .iter_mut()
            .filter(|(_, entry)| !entry.deleted)
            .find(|(entry_name, _)| *entry_name == &name);

        Ok(entry.map(|(_, entry)| &mut entry.link))
    }

    /// Adds or updates an entry in the directory, handling versioning.
    ///
    /// This method ensures proper versioning:
    /// - For new entries: Adds the entry as is
    /// - For existing entries: Sets the previous version appropriately
    /// - For deleted entries: Restores them with proper versioning
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, File};
    /// use monoutils_store::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut dir = Dir::new(store.clone());
    ///
    /// // Add new file
    /// dir.put_adapted_entry("test.txt", File::new(store.clone()).into()).await?;
    ///
    /// // Store the first version
    /// let dir_cid = dir.store().await?;
    /// let mut loaded_dir = Dir::load(&dir_cid, store.clone()).await?;
    /// let first_cid = loaded_dir.get_entry("test.txt")?.unwrap().get_cid().unwrap().clone();
    ///
    /// // Update with new version
    /// loaded_dir.put_adapted_entry("test.txt", File::new(store.clone()).into()).await?;
    ///
    /// // The second version should have the first as its previous
    /// let entry = loaded_dir.get_entry("test.txt")?.unwrap();
    /// let entity = entry.resolve_entity(store).await?;
    /// assert_eq!(entity.get_previous(), Some(&first_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put_adapted_entry(
        &mut self,
        name: impl AsRef<str>,
        mut link: EntityCidLink<S>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        let name = Utf8UnixPathSegment::from_str(name.as_ref())?;
        let inner = Arc::make_mut(&mut self.inner);

        // Resolve the link to an entity
        let entity = link.resolve_entity_mut(inner.store.clone()).await?;

        // Unset the previous version
        entity.set_previous(None);

        // Handle existing entry if present
        if let Some(existing_entry) = inner.entries.get(&name) {
            if let Some(existing_entity) = existing_entry.link.get_entity() {
                // If we can get the entity without resolving, use its previous
                entity.set_previous(existing_entity.get_previous().cloned());
            } else if let Some(existing_cid) = existing_entry.link.get_cid() {
                // Otherwise use the CID as previous
                entity.set_previous(Some(*existing_cid));
            }
        }

        // Update the modified timestamp
        let now = Utc::now();
        entity.get_metadata_mut().set_modified_at(now);
        inner.metadata.set_modified_at(now);

        // Add or update the entry
        inner.entries.insert(
            name,
            Entry {
                deleted: false,
                link,
            },
        );

        Ok(())
    }

    /// Adds an [`Entity`] and its associated name in the directory's entries.
    ///
    /// This is a convenience wrapper around [`put_adapted_entry`][Self::put_adapted_entry] that handles
    /// converting the entity into an [`EntityCidLink`]. It provides the same versioning guarantees:
    ///
    /// - Verifies no previous version is set on the incoming entity
    /// - Properly chains versions with any existing entry
    /// - Updates metadata timestamps
    /// - Handles deleted entry restoration
    ///
    /// See [`put_adapted_entry`][Self::put_adapted_entry] for more details on the versioning behavior.
    #[inline]
    pub async fn put_adapted_entity(
        &mut self,
        name: impl AsRef<str>,
        entity: impl Into<Entity<S>>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_adapted_entry(name, EntityCidLink::from(entity.into()))
            .await
    }

    /// Adds a [`Dir`] and its associated name in the directory's entries.
    ///
    /// This is a convenience wrapper around [`put_adapted_entry`][Self::put_adapted_entry] that handles
    /// converting the directory into an [`EntityCidLink`]. It provides the same versioning guarantees:
    ///
    /// - Verifies no previous version is set on the incoming directory
    /// - Properly chains versions with any existing entry
    /// - Updates metadata timestamps
    /// - Handles deleted entry restoration
    ///
    /// See [`put_adapted_entry`][Self::put_adapted_entry] for more details on the versioning behavior.
    #[inline]
    pub async fn put_adapted_dir(&mut self, name: impl AsRef<str>, dir: Dir<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_adapted_entry(name, EntityCidLink::from(dir)).await
    }

    /// Adds a [`File`] and its associated name in the directory's entries.
    ///
    /// This is a convenience wrapper around [`put_adapted_entry`][Self::put_adapted_entry] that handles
    /// converting the file into an [`EntityCidLink`]. It provides the same versioning guarantees:
    ///
    /// - Verifies no previous version is set on the incoming file
    /// - Properly chains versions with any existing entry
    /// - Updates metadata timestamps
    /// - Handles deleted entry restoration
    ///
    /// See [`put_adapted_entry`][Self::put_adapted_entry] for more details on the versioning behavior.
    #[inline]
    pub async fn put_adapted_file(&mut self, name: impl AsRef<str>, file: File<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_adapted_entry(name, EntityCidLink::from(file))
            .await
    }

    /// Adds a [`SymCidLink`] and its associated name in the directory's entries.
    ///
    /// This is a convenience wrapper around [`put_adapted_entry`][Self::put_adapted_entry] that handles
    /// converting the symbolic link into an [`EntityCidLink`]. It provides the same versioning guarantees:
    ///
    /// - Verifies no previous version is set on the incoming symlink
    /// - Properly chains versions with any existing entry
    /// - Updates metadata timestamps
    /// - Handles deleted entry restoration
    ///
    /// See [`put_adapted_entry`][Self::put_adapted_entry] for more details on the versioning behavior.
    #[inline]
    pub async fn put_adapted_symcidlink(
        &mut self,
        name: impl AsRef<str>,
        symlink: SymCidLink<S>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_adapted_entry(name, EntityCidLink::from(symlink))
            .await
    }

    /// Adds a [`SymPathLink`] and its associated name in the directory's entries.
    ///
    /// This is a convenience wrapper around [`put_adapted_entry`][Self::put_adapted_entry] that handles
    /// converting the symbolic link into an [`EntityCidLink`]. It provides the same versioning guarantees:
    ///
    /// - Verifies no previous version is set on the incoming symlink
    /// - Properly chains versions with any existing entry
    /// - Updates metadata timestamps
    /// - Handles deleted entry restoration
    ///
    /// See [`put_adapted_entry`][Self::put_adapted_entry] for more details on the versioning behavior.
    #[inline]
    pub async fn put_adapted_sympathlink(
        &mut self,
        name: impl AsRef<str>,
        symlink: SymPathLink<S>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_adapted_entry(name, EntityCidLink::from(symlink))
            .await
    }

    /// Gets the [`Entity`] with the associated name from the directory's entries.
    pub async fn get_entity(&self, name: impl AsRef<str>) -> FsResult<Option<&Entity<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entry(name)? {
            Some(link) => Ok(Some(link.resolve_entity(self.inner.store.clone()).await?)),
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
            Some(link) => Ok(Some(link.resolve_entity_mut(store).await?)),
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

    /// Tries to create a new `Dir` from a serializable representation.
    pub fn from_serializable(
        serializable: DirSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        let entries: HashMap<_, _> = serializable
            .entries
            .into_iter()
            .map(|(segment, (deleted, cid))| {
                Ok((
                    segment.parse()?,
                    Entry {
                        deleted,
                        link: Link::from(cid),
                    },
                ))
            })
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
        for (k, v) in self.inner.entries.iter() {
            entries.insert(
                k.to_string(),
                (
                    v.deleted,
                    v.link.resolve_cid().await.map_err(StoreError::custom)?,
                ),
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

    pub(crate) fn set_previous(&mut self, previous: Option<Cid>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.previous = previous;
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
            .field("previous", &self.get_previous())
            .finish()
    }
}

impl<S> Debug for Entry<S>
where
    S: IpldStore + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("deleted", &self.deleted)
            .field("link", &self.link)
            .finish()
    }
}

impl IpldReferences for DirSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(self.entries.values().map(|(_, cid)| cid))
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
    async fn test_dir_stores_loads() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create a file
        let file = File::new(store.clone());
        let file_name = "file1";

        // Add file directly
        dir.put_adapted_file(file_name, file).await?;

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
            dir_metadata.get_created_at(),
            loaded_dir_metadata.get_created_at()
        );
        assert_eq!(
            dir_metadata.get_modified_at(),
            loaded_dir_metadata.get_modified_at()
        );

        // Assert that the number of entries is the same
        assert_eq!(dir.get_entries().count(), loaded_dir.get_entries().count());

        // Assert that the entry exists in the loaded directory
        assert!(loaded_dir.get_entry(&file_name)?.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_has_entry() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create a file
        let file = File::new(store.clone());
        let file_name = "example.txt";

        // Add file directly
        dir.put_adapted_file(file_name, file).await?;

        assert!(dir.has_entry(file_name)?);
        assert!(!dir.has_entry("nonexistent.txt")?);

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_remove_entry() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create a file
        let file = File::new(store.clone());
        let file_name = "example.txt";

        // Add file directly
        dir.put_adapted_file(file_name, file).await?;
        assert!(dir.has_entry(file_name)?);

        dir.remove_entry(file_name)?;
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
        assert_eq!(*metadata.get_sync_type(), SyncType::Default);

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_is_empty() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());
        assert!(dir.is_empty());

        // Create a file
        let file = File::new(store.clone());
        let file_name = "example.txt";

        // Add file directly
        dir.put_adapted_file(file_name, file).await?;
        assert!(!dir.is_empty());

        dir.remove_entry(file_name)?;
        assert!(dir.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_entries() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create two files
        let file1 = File::new(store.clone());
        let file2 = File::new(store.clone());

        let file1_name = "file1.txt";
        let file2_name = "file2.txt";

        // Add files directly
        dir.put_adapted_file(file1_name, file1).await?;
        dir.put_adapted_file(file2_name, file2).await?;

        let entries: Vec<_> = dir.get_entries().collect();
        assert_eq!(entries.len(), 2);

        let entry1 = entries.iter().find(|(name, _)| name.as_str() == file1_name);
        let entry2 = entries.iter().find(|(name, _)| name.as_str() == file2_name);

        // Just verify the entries exist since their CIDs will be different
        assert!(entry1.is_some());
        assert!(entry2.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_initial_load_cid() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Initially, the CID is not set
        assert!(dir.get_initial_load_cid().is_none());

        // Store and checkpoint the directory
        let stored_cid = dir.checkpoint().await?;

        // Now the initial load CID is set
        assert_eq!(dir.get_initial_load_cid(), Some(&stored_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_get_previous() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Initially, there's no previous version
        assert!(dir.get_previous().is_none());

        // Store initial version
        let first_cid = dir.checkpoint().await?;

        // Create a new file and add it
        let file = File::new(store.clone());
        dir.put_adapted_file("new_file", file).await?;

        // Store the new version
        let second_cid = dir.checkpoint().await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(dir.get_previous(), Some(&first_cid));
        assert_eq!(dir.get_initial_load_cid(), Some(&second_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_put_adapted_entry_new_entry() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Add the file using its CID
        dir.put_adapted_entry("file.txt", File::new(store.clone()).into())
            .await?;

        // Since this is a new entry, it should not have a previous version
        let entry = dir.get_entry("file.txt")?.unwrap();
        assert!(entry.resolve_entity(store).await?.get_previous().is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_put_adapted_entry_existing_decoded_entry() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create first version of file.txt
        dir.put_adapted_entry("file.txt", File::new(store.clone()).into())
            .await?;

        // Persist the directory
        dir.checkpoint().await?;
        let file_cid = dir
            .get_entry("file.txt")?
            .unwrap()
            .get_cid()
            .unwrap()
            .clone();

        // Create second version
        dir.put_adapted_entry("file.txt", File::new(store.clone()).into())
            .await?;

        // Verify the entry exists and has the initial file's CID as previous
        let entry = dir.get_entry("file.txt")?.unwrap();
        let entity = entry.resolve_entity(store).await?;
        assert_eq!(entity.get_previous(), Some(&file_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_put_adapted_entry_restore_deleted() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create and add a file
        dir.put_adapted_entry("file.txt", File::new(store.clone()).into())
            .await?;

        // Delete the entry
        dir.remove_entry("file.txt")?;
        assert!(!dir.has_entry("file.txt")?);

        // Persist the directory
        dir.checkpoint().await?;
        let file_cid = dir
            .get_all_entries()
            .find(|(name, _)| name.as_str() == "file.txt")
            .unwrap()
            .1
            .get_cid()
            .unwrap()
            .clone();

        // Create new version and add it
        dir.put_adapted_entry("file.txt", File::new(store.clone()).into())
            .await?;

        // Verify the entry is restored and visible
        assert!(dir.has_entry("file.txt")?);
        let entry = dir.get_entry("file.txt")?.unwrap();
        let entity = entry.resolve_entity(store).await?;
        assert_eq!(entity.get_previous(), Some(&file_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_put_adapted_entry_updates_timestamps() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Record initial directory modified time
        let dir_initial_modified = *dir.get_metadata().get_modified_at();

        // Wait a moment to ensure timestamps will be different
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Create and add a file
        let file = File::new(store.clone());
        dir.put_adapted_entry("file.txt", file.into()).await?;

        // Verify directory's modified time was updated
        assert!(*dir.get_metadata().get_modified_at() > dir_initial_modified);

        // Get the entry and verify its modified time
        let entry = dir.get_entry("file.txt")?.unwrap();
        let entity = entry.resolve_entity(store).await?;
        assert!(*entity.get_metadata().get_modified_at() > dir_initial_modified);

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use find::*;
pub use segment::*;

use super::SymPathLink;

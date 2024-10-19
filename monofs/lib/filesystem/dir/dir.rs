use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Debug},
    sync::Arc,
};

use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{
    de::{self, DeserializeSeed},
    Deserialize, Deserializer, Serialize,
};

use crate::{
    file::File,
    filesystem::{entity::Entity, kind::EntityType},
    softlink::SoftLink,
    EntityCidLink, FsError, FsResult, Link, Metadata,
};

use super::Utf8UnixPathSegment;

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
    inner: Arc<DirInner<S>>,
}

#[derive(Clone)]
struct DirInner<S>
where
    S: IpldStore,
{
    /// Directory metadata.
    pub(crate) metadata: Metadata,

    /// The store used to persist blocks in the directory.
    pub(crate) store: S,

    /// The entries in the directory.
    pub(crate) entries: HashMap<Utf8UnixPathSegment, EntityCidLink<S>>,
}

//--------------------------------------------------------------------------------------------------
// Types: *
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DirSerializable {
    metadata: Metadata,
    entries: BTreeMap<String, Cid>,
}

pub(crate) struct DirDeserializeSeed<S> {
    pub(crate) store: S,
}

//--------------------------------------------------------------------------------------------------
// Methods: Dir
//--------------------------------------------------------------------------------------------------

impl<S> Dir<S>
where
    S: IpldStore,
{
    /// Creates a new directory with the given store.
    pub fn new(store: S) -> Self {
        Self {
            inner: Arc::new(DirInner {
                metadata: Metadata::new(EntityType::Dir),
                entries: HashMap::new(),
                store,
            }),
        }
    }

    /// Adds a [`EntityCidLink`] and its associated name in the directory's entries.
    pub fn put_entry(&mut self, name: Utf8UnixPathSegment, link: EntityCidLink<S>) -> FsResult<()> {
        let inner = Arc::make_mut(&mut self.inner);
        inner.entries.insert(name, link);
        Ok(())
    }

    /// Adds an [`Entity`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_entity(&mut self, name: Utf8UnixPathSegment, entity: Entity<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(entity))
    }

    /// Adds a [`Dir`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_dir(&mut self, name: Utf8UnixPathSegment, dir: Dir<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(dir))
    }

    /// Adds a [`File`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_file(&mut self, name: Utf8UnixPathSegment, file: File<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(file))
    }

    /// Adds a [`SoftLink`] and its associated name in the directory's entries.
    #[inline]
    pub fn put_softlink(&mut self, name: Utf8UnixPathSegment, softlink: SoftLink<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put_entry(name, EntityCidLink::from(softlink))
    }

    /// Gets the [`EntityCidLink`] with the given name from the directory's entries.
    #[inline]
    pub fn get_entry(&self, name: &Utf8UnixPathSegment) -> Option<&EntityCidLink<S>> {
        self.inner.entries.get(name)
    }

    /// Gets the [`EntityCidLink`] with the given name from the directory's entries.
    #[inline]
    pub fn get_entry_mut(&mut self, name: &Utf8UnixPathSegment) -> Option<&mut EntityCidLink<S>> {
        let inner = Arc::make_mut(&mut self.inner);
        inner.entries.get_mut(name)
    }

    /// Gets the [`Entity`] with the associated name from the directory's entries.
    pub async fn get_entity(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&Entity<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entry(name) {
            Some(link) => Ok(Some(link.resolve_entity(self.inner.store.clone()).await?)),
            None => Ok(None),
        }
    }

    /// Gets the [`Entity`] with the associated name from the directory's entries.
    pub async fn get_entity_mut(
        &mut self,
        name: &Utf8UnixPathSegment,
    ) -> FsResult<Option<&mut Entity<S>>>
    where
        S: Send + Sync,
    {
        let store = self.inner.store.clone();
        match self.get_entry_mut(name) {
            Some(link) => Ok(Some(link.resolve_entity_mut(store).await?)),
            None => Ok(None),
        }
    }

    /// Gets the [`Dir`] with the associated name from the directory's entries.
    pub async fn get_dir(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::Dir(dir)) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`Dir`] with the associated name from the directory's entries.
    pub async fn get_dir_mut(&mut self, name: &Utf8UnixPathSegment) -> FsResult<Option<&mut Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity_mut(name).await? {
            Some(Entity::Dir(dir)) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] with the associated name from the directory's entries.
    pub async fn get_file(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::File(file)) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] with the associated name from the directory's entries.
    pub async fn get_file_mut(
        &mut self,
        name: &Utf8UnixPathSegment,
    ) -> FsResult<Option<&mut File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity_mut(name).await? {
            Some(Entity::File(file)) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`SoftLink`] with the associated name from the directory's entries.
    pub async fn get_softlink(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&SoftLink<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::SoftLink(softlink)) => Ok(Some(softlink)),
            _ => Ok(None),
        }
    }

    /// Gets the [`SoftLink`] with the associated name from the directory's entries.
    pub async fn get_softlink_mut(
        &mut self,
        name: &Utf8UnixPathSegment,
    ) -> FsResult<Option<&mut SoftLink<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity_mut(name).await? {
            Some(Entity::SoftLink(softlink)) => Ok(Some(softlink)),
            _ => Ok(None),
        }
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata {
        &self.inner.metadata
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
    pub fn is_empty(&self) -> bool {
        self.inner.entries.is_empty()
    }

    /// Deserializes to a `Dir` using an arbitrary deserializer and store.
    pub fn deserialize_with<'de>(
        deserializer: impl Deserializer<'de, Error: Into<FsError>>,
        store: S,
    ) -> FsResult<Self> {
        DirDeserializeSeed::new(store)
            .deserialize(deserializer)
            .map_err(Into::into)
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub(crate) fn try_from_serializable(serializable: DirSerializable, store: S) -> FsResult<Self> {
        let entries: HashMap<_, _> = serializable
            .entries
            .into_iter()
            .map(|(segment, cid)| Ok((segment.parse()?, Link::from(cid))))
            .collect::<FsResult<_>>()?;

        Ok(Dir {
            inner: Arc::new(DirInner {
                metadata: serializable.metadata,
                store,
                entries,
            }),
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Methods: DirDeserializeSeed
//--------------------------------------------------------------------------------------------------

impl<S> DirDeserializeSeed<S> {
    fn new(store: S) -> Self {
        Self { store }
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
        let mut entries = BTreeMap::new();
        for (k, v) in self.get_entries() {
            entries.insert(
                k.to_string(),
                v.resolve_cid().await.map_err(StoreError::custom)?,
            );
        }

        let serializable = DirSerializable {
            metadata: self.inner.metadata.clone(),
            entries,
        };

        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable: DirSerializable = store.get_node(cid).await?;
        Dir::try_from_serializable(serializable, store).map_err(StoreError::custom)
    }
}

impl<S> Debug for Dir<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dir")
            .field("metadata", &self.inner.metadata)
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

impl<'de, S> DeserializeSeed<'de> for DirDeserializeSeed<S>
where
    S: IpldStore,
{
    type Value = Dir<S>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let serializable = DirSerializable::deserialize(deserializer)?;
        Dir::try_from_serializable(serializable, self.store).map_err(de::Error::custom)
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
        let store = MemoryStore::default();
        let mut dir = Dir::new(store);

        let file1_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
        let file2_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        let file1_name: Utf8UnixPathSegment = "file1".parse()?;
        let file2_name: Utf8UnixPathSegment = "file2".parse()?;

        dir.put_entry(file1_name.clone(), file1_cid.clone().into())?;
        dir.put_entry(file2_name.clone(), file2_cid.clone().into())?;

        assert_eq!(dir.inner.entries.len(), 2);
        assert_eq!(
            dir.get_entry(&file1_name).unwrap().get_cid(),
            Some(&file1_cid)
        );
        assert_eq!(
            dir.get_entry(&file2_name).unwrap().get_cid(),
            Some(&file2_cid)
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_stores_loads() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        let file_name: Utf8UnixPathSegment = "file1".parse()?;
        let file_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        dir.put_entry(file_name.clone(), file_cid.clone().into())?;

        let cid = dir.store().await?;
        let loaded_dir = Dir::load(&cid, store.clone()).await?;

        // Assert that the metadata is the same
        assert_eq!(dir.get_metadata(), loaded_dir.get_metadata());

        // Assert that the number of entries is the same
        assert_eq!(dir.get_entries().count(), loaded_dir.get_entries().count());

        // Assert that the entry we added exists in the loaded directory
        let loaded_entry = loaded_dir
            .get_entry(&file_name)
            .expect("Entry should exist");

        assert_eq!(loaded_entry.get_cid(), Some(&file_cid));

        Ok(())
    }
}

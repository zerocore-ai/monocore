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
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{
    file::File,
    filesystem::{entity::Entity, kind::EntityType},
    symlink::Symlink,
    EntityCidLink, FsError, FsResult, Link, Metadata, Resolvable,
};

use super::Utf8UnixPathSegment;

//--------------------------------------------------------------------------------------------------
// Types: Dir
//--------------------------------------------------------------------------------------------------

/// Represents a directory node in the `monofs` file system.
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
    pub fn put(&mut self, name: Utf8UnixPathSegment, link: EntityCidLink<S>) -> FsResult<()> {
        let inner = Arc::make_mut(&mut self.inner);
        inner.entries.insert(name, link);
        Ok(())
    }

    /// Adds an [`Entity`] and its associated name in the directory's entries.
    #[inline]
    pub async fn put_entity(&mut self, name: Utf8UnixPathSegment, entity: Entity<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put(name, EntityCidLink::from_entity(entity).await?)
    }

    /// Adds a [`Dir`] and its associated name in the directory's entries.
    #[inline]
    pub async fn put_dir(&mut self, name: Utf8UnixPathSegment, dir: Dir<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put(name, EntityCidLink::from_dir(dir).await?)
    }

    /// Adds a [`File`] and its associated name in the directory's entries.
    #[inline]
    pub async fn put_file(&mut self, name: Utf8UnixPathSegment, file: File<S>) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put(name, EntityCidLink::from_file(file).await?)
    }

    /// Adds a [`Symlink`] and its associated name in the directory's entries.
    #[inline]
    pub async fn put_symlink(
        &mut self,
        name: Utf8UnixPathSegment,
        symlink: Symlink<S>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        self.put(name, EntityCidLink::from_symlink(symlink).await?)
    }

    /// Gets the [`EntityCidLink`] with the given name from the directory's entries.
    #[inline]
    pub fn get(&self, name: &Utf8UnixPathSegment) -> Option<&EntityCidLink<S>> {
        self.inner.entries.get(name)
    }

    /// Gets the entity with the provided name from the directory's entries, resolving it if necessary.
    pub async fn get_entity(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&Entity<S>>>
    where
        S: Send + Sync,
    {
        match self.get(name) {
            Some(link) => Ok(Some(link.resolve(self.inner.store.clone()).await?)),
            None => Ok(None),
        }
    }

    /// Gets the [`Dir`] with the provided name from the directory's entries, resolving it if necessary.
    pub async fn get_dir(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::Dir(dir)) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] with the provided name from the directory's entries, resolving it if necessary.
    pub async fn get_file(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::File(file)) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`Symlink`] with the provided name from the directory's entries, resolving it if necessary.
    pub async fn get_symlink(&self, name: &Utf8UnixPathSegment) -> FsResult<Option<&Symlink<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(name).await? {
            Some(Entity::Symlink(symlink)) => Ok(Some(symlink)),
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

    /// Change the store used to persist the directory.
    pub fn use_store<T>(self, store: T) -> Dir<T>
    where
        T: IpldStore,
    {
        let inner = match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner,
            Err(arc) => (*arc).clone(),
        };

        Dir {
            inner: Arc::new(DirInner {
                metadata: inner.metadata,
                entries: inner
                    .entries
                    .into_iter()
                    .map(|(k, v)| (k, v.use_store(&store)))
                    .collect(),
                store,
            }),
        }
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

impl<S> IpldReferences for Dir<S>
where
    S: IpldStore + Send + Sync,
{
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(self.get_entries().map(|(_, v)| v.get_cid()))
    }
}

impl<S> Storable<S> for Dir<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        self.inner.store.put_node(self).await
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
                    .map(|(_, v)| v.get_cid())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl<S> Serialize for Dir<S>
where
    S: IpldStore,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        let serializable = DirSerializable {
            metadata: self.inner.metadata.clone(),
            entries: self
                .get_entries()
                .map(|(k, v)| (k.to_string(), *v.get_cid()))
                .collect(),
        };

        serializable.serialize(serializer)
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

impl<S> PartialEq for Dir<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<S> PartialEq for DirInner<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
            && self.entries.len() == other.entries.len()
            && self.entries == other.entries
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

        dir.put(
            "file1".parse()?,
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?,
        )?;

        dir.put(
            "file2".parse()?,
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?,
        )?;

        assert_eq!(dir.inner.entries.len(), 2);
        assert_eq!(
            dir.get(&"file1".parse()?).unwrap().get_cid(),
            &"bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?
        );
        assert_eq!(
            dir.get(&"file2".parse()?).unwrap().get_cid(),
            &"bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_stores_loads() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        dir.put(
            "file1".parse()?,
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?,
        )?;

        let cid = dir.store().await?;
        let loaded_dir = Dir::load(&cid, store.clone()).await?;

        assert_eq!(dir, loaded_dir);

        Ok(())
    }

    // #[tokio::test]
    // async fn test_get_or_create_entity() -> anyhow::Result<()> {
    //     let store = MemoryStore::default();
    //     let root = Dir::new(store.clone());

    //     // Test creating a new file
    //     let (entity, name, pathdirs) = root
    //         .get_or_create_entity(&Path::try_from("/new_file.txt")?, true)
    //         .await?;
    //     assert!(matches!(entity, Entity::File(_)));
    //     assert_eq!(name, Some("new_file.txt".parse()?));
    //     assert_eq!(pathdirs.len(), 0);

    //     // Test creating a new directory
    //     let (entity, name, pathdirs) = root
    //         .get_or_create_entity(&Path::try_from("/new_dir")?, false)
    //         .await?;
    //     assert!(matches!(entity, Entity::Dir(_)));
    //     assert_eq!(name, Some("new_dir".parse()?));
    //     assert_eq!(pathdirs.len(), 0);

    //     // Test creating a nested structure
    //     let (entity, name, pathdirs) = root
    //         .get_or_create_entity(&Path::try_from("/parent/child/file.txt")?, true)
    //         .await?;
    //     assert!(matches!(entity, Entity::File(_)));
    //     assert_eq!(name, Some("file.txt".parse()?));
    //     assert_eq!(pathdirs.len(), 2);
    //     assert_eq!(pathdirs[0].1, "parent".parse()?);
    //     assert_eq!(pathdirs[1].1, "child".parse()?);

    //     // Test getting an existing entity
    //     let mut root = root;
    //     let file = File::new(store.clone());
    //     let file_cid = file.store().await?;
    //     root.put("existing_file.txt", file_cid)?;

    //     let (entity, name, pathdirs) = root
    //         .get_or_create_entity(&Path::try_from("/existing_file.txt")?, true)
    //         .await?;
    //     assert!(matches!(entity, Entity::File(_)));
    //     assert_eq!(name, Some("existing_file.txt".parse()?));
    //     assert_eq!(pathdirs.len(), 0);

    //     Ok(())
    // }
}

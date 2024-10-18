use std::{
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

use crate::{filesystem::kind::EntityType, FsError, FsResult, Metadata};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a file node in the `monofs` file system.
///
/// ## Important
///
/// Entities in `monofs` are designed to be immutable and clone-on-write meaning writes create
/// forks of the entity.
#[derive(Clone)]
pub struct File<S>
where
    S: IpldStore,
{
    inner: Arc<FileInner<S>>,
}

#[derive(Clone)]
struct FileInner<S>
where
    S: IpldStore,
{
    /// File metadata.
    pub(crate) metadata: Metadata,

    /// File content. If the file is empty, this will be `None`.
    pub(crate) content: Option<Cid>,

    /// The store used to persist blocks in the file.
    pub(crate) store: S,
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileSerializable {
    metadata: Metadata,
    content: Option<Cid>,
}

pub(crate) struct FileDeserializeSeed<S> {
    pub(crate) store: S,
}

//--------------------------------------------------------------------------------------------------
// Methods: File
//--------------------------------------------------------------------------------------------------

impl<S> File<S>
where
    S: IpldStore,
{
    /// Creates a new file.
    pub fn new(store: S) -> Self {
        Self {
            inner: Arc::new(FileInner {
                metadata: Metadata::new(EntityType::File),
                content: None,
                store,
            }),
        }
    }

    /// Returns the content of the file.
    pub fn get_content(&self) -> Option<&Cid> {
        self.inner.content.as_ref()
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata {
        &self.inner.metadata
    }

    /// Returns the store used to persist the file.
    pub fn get_store(&self) -> &S {
        &self.inner.store
    }

    /// Returns `true` if the file is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.content.is_none()
    }

    /// Truncates the file to zero bytes.
    pub fn truncate(&mut self) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.content = None;
    }

    /// Change the store used to persist the file.
    pub fn use_store<T>(self, store: T) -> File<T>
    where
        T: IpldStore,
    {
        let inner = match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner,
            Err(arc) => (*arc).clone(),
        };

        File {
            inner: Arc::new(FileInner {
                metadata: inner.metadata,
                content: inner.content,
                store,
            }),
        }
    }

    /// Deserializes to a `Dir` using an arbitrary deserializer and store.
    pub fn deserialize_with<'de>(
        deserializer: impl Deserializer<'de, Error: Into<FsError>>,
        store: S,
    ) -> FsResult<Self> {
        FileDeserializeSeed::new(store)
            .deserialize(deserializer)
            .map_err(Into::into)
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub(crate) fn try_from_serializable(
        serializable: FileSerializable,
        store: S,
    ) -> FsResult<Self> {
        Ok(File {
            inner: Arc::new(FileInner {
                metadata: serializable.metadata,
                content: serializable.content,
                store,
            }),
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Methods: FileDeserializeSeed
//--------------------------------------------------------------------------------------------------

impl<S> FileDeserializeSeed<S> {
    fn new(store: S) -> Self {
        Self { store }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: File
//--------------------------------------------------------------------------------------------------

impl<S> IpldReferences for File<S>
where
    S: IpldStore,
{
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        match self.inner.content.as_ref() {
            Some(cid) => Box::new(std::iter::once(cid)),
            None => Box::new(std::iter::empty()),
        }
    }
}

impl<S> Serialize for File<S>
where
    S: IpldStore,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        let serializable = FileSerializable {
            metadata: self.inner.metadata.clone(),
            content: self.inner.content,
        };

        serializable.serialize(serializer)
    }
}

impl<S> Storable<S> for File<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        self.inner.store.put_node(self).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        File::try_from_serializable(serializable, store).map_err(StoreError::custom)
    }
}

impl<S> Debug for File<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File")
            .field("metadata", &self.inner.metadata)
            .field("content", &self.inner.content)
            .finish()
    }
}

impl<S> PartialEq for File<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner.metadata == other.inner.metadata && self.inner.content == other.inner.content
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations: FileDeserializeSeed
//--------------------------------------------------------------------------------------------------

impl<'de, S> DeserializeSeed<'de> for FileDeserializeSeed<S>
where
    S: IpldStore,
{
    type Value = File<S>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let serializable = FileSerializable::deserialize(deserializer)?;
        File::try_from_serializable(serializable, self.store).map_err(de::Error::custom)
    }
}

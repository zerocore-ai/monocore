use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use bytes::Bytes;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{
    de::{self, DeserializeSeed},
    Deserialize, Deserializer, Serialize,
};

use crate::filesystem::{kind::EntityType, FsError, FsResult, Metadata};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a file node in the `monofs` _immutable_ file system.
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
    /// Creates a new empty file.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::MemoryStore;
    ///
    /// let store = MemoryStore::default();
    /// let file = File::new(store);
    ///
    /// assert!(file.is_empty());
    /// ```
    pub fn new(store: S) -> Self {
        Self {
            inner: Arc::new(FileInner {
                metadata: Metadata::new(EntityType::File),
                content: None,
                store,
            }),
        }
    }

    /// Creates a new file with the given content.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let file = File::with_content(store, b"Hello, World!".to_vec()).await;
    ///
    /// assert!(!file.is_empty());
    /// assert!(file.get_content().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_content(store: S, content: impl Into<Bytes> + Send) -> Self {
        let cid = store.put_raw_block(content).await.unwrap();

        Self {
            inner: Arc::new(FileInner {
                metadata: Metadata::new(EntityType::File),
                content: Some(cid),
                store,
            }),
        }
    }

    /// Returns the content of the file.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let file = File::with_content(store, b"Hello, World!".to_vec()).await;
    ///
    /// assert!(file.get_content().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_content(&self) -> Option<&Cid> {
        self.inner.content.as_ref()
    }

    /// Sets the content of the file.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::{MemoryStore, ipld::cid::Cid};
    ///
    /// let store = MemoryStore::default();
    /// let mut file = File::new(store);
    ///
    /// let content_cid = Cid::default();
    /// file.set_content(Some(content_cid));
    ///
    /// assert!(!file.is_empty());
    /// assert_eq!(file.get_content(), Some(&content_cid));
    /// ```
    pub fn set_content(&mut self, content: Option<Cid>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.content = content;
    }

    /// Returns the metadata for the file.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{File, EntityType};
    /// use monoutils_store::MemoryStore;
    ///
    /// let store = MemoryStore::default();
    /// let file = File::new(store);
    ///
    /// assert_eq!(file.get_metadata().get_entity_type(), &EntityType::File);
    /// ```
    pub fn get_metadata(&self) -> &Metadata {
        &self.inner.metadata
    }

    /// Returns the store used to persist the file.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::{MemoryStore, IpldStore};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let file = File::new(store);
    ///
    /// assert!(file.get_store().is_empty().await?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_store(&self) -> &S {
        &self.inner.store
    }

    /// Returns `true` if the file is empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::MemoryStore;
    ///
    /// let store = MemoryStore::default();
    /// let file = File::new(store);
    ///
    /// assert!(file.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.content.is_none()
    }

    /// Truncates the file to zero bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::{MemoryStore, ipld::cid::Cid};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut file = File::with_content(store, b"Hello, World!".to_vec()).await;
    ///
    /// assert!(!file.is_empty());
    ///
    /// file.truncate();
    ///
    /// assert!(file.is_empty());
    /// assert!(file.get_content().is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn truncate(&mut self) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.content = None;
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

impl<S> Storable<S> for File<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let serializable = FileSerializable {
            metadata: self.inner.metadata.clone(),
            content: self.inner.content,
        };

        self.inner.store.put_node(&serializable).await
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

impl IpldReferences for FileSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        match self.content.as_ref() {
            Some(cid) => Box::new(std::iter::once(cid)),
            None => Box::new(std::iter::empty()),
        }
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::MemoryStore;

    use super::*;

    #[test]
    fn test_file_new() {
        let file = File::new(MemoryStore::default());

        assert!(file.is_empty());
        assert_eq!(file.get_metadata().get_entity_type(), &EntityType::File);
        assert!(file.get_content().is_none());
    }

    #[tokio::test]
    async fn test_file_with_content() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let file = File::with_content(store.clone(), b"Hello, World!".to_vec()).await;
        assert!(!file.is_empty());

        let content_cid = file.get_content().unwrap();
        assert_eq!(
            store.get_raw_block(content_cid).await?,
            Bytes::from(b"Hello, World!".to_vec())
        );

        Ok(())
    }

    #[test]
    fn test_file_set_content() {
        let mut file = File::new(MemoryStore::default());

        let content_cid = Cid::default();
        file.set_content(Some(content_cid));

        assert!(!file.is_empty());
        assert_eq!(file.get_content(), Some(&content_cid));
    }

    #[test]
    fn test_file_truncate() {
        let mut file = File::new(MemoryStore::default());

        let content_cid = Cid::default();
        file.set_content(Some(content_cid));
        assert!(!file.is_empty());

        file.truncate();
        assert!(file.is_empty());
        assert!(file.get_content().is_none());
    }

    #[test]
    fn test_file_equality() {
        let store = MemoryStore::default();
        let file1 = File::new(store.clone());
        let file2 = File::new(store.clone());
        let mut file3 = File::new(store);

        assert_eq!(file1, file2);

        let content_cid = Cid::default();
        file3.set_content(Some(content_cid));

        assert_ne!(file1, file3);
    }

    #[tokio::test]
    async fn test_file_store_and_load() {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());

        let content_cid = Cid::default();
        file.set_content(Some(content_cid));

        let stored_cid = file.store().await.unwrap();
        let loaded_file = File::load(&stored_cid, store).await.unwrap();

        assert_eq!(file, loaded_file);
    }
}

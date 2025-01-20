mod io;

use std::{
    fmt::{self, Debug},
    sync::{Arc, OnceLock},
};

use chrono::Utc;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;

use crate::filesystem::{kind::EntityType, FsResult, Metadata, MetadataSerializable};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The type identifier for files.
pub const FILE_TYPE_TAG: &str = "monofs.file";

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
    /// The CID of the file when it is initially loaded from the store.
    ///
    /// It is not initialized if the file was not loaded from the store.
    initial_load_cid: OnceLock<Cid>,

    /// The CID of the previous version of the directory if there is one.
    previous: Option<Cid>,

    /// File metadata.
    metadata: Metadata<S>,

    /// File content. If the file is empty, this will be `None`.
    content: Option<Cid>,

    /// The store used to persist blocks in the file.
    store: S,
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

/// A serializable representation of [`File`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSerializable {
    /// The type of the entity.
    pub r#type: String,

    /// The metadata of the file.
    metadata: MetadataSerializable,

    /// The content of the file.
    content: Option<Cid>,

    /// The CID of the previous version of the file if there is one.
    previous: Option<Cid>,
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
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let file = File::new(store);
    ///
    /// assert!(file.is_empty().await?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(store: S) -> Self {
        Self {
            inner: Arc::new(FileInner {
                initial_load_cid: OnceLock::new(),
                previous: None,
                metadata: Metadata::new(EntityType::File, store.clone()),
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
    /// let file = File::with_content(store, b"Hello, World!".as_slice()).await?;
    ///
    /// assert!(!file.is_empty().await?);
    /// assert!(file.get_content().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_content(store: S, content: impl AsyncRead + Send + Sync) -> FsResult<Self> {
        let cid = store.put_bytes(content).await?;

        Ok(Self {
            inner: Arc::new(FileInner {
                initial_load_cid: OnceLock::new(),
                previous: None,
                metadata: Metadata::new(EntityType::File, store.clone()),
                content: Some(cid),
                store,
            }),
        })
    }

    /// Returns the CID of the file when it was initially loaded from the store.
    ///
    /// It returns `None` if the file was not loaded from the store.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let file = File::new(store.clone());
    ///
    /// // Initially, the CID is not set
    /// assert!(file.get_initial_load_cid().is_none());
    ///
    /// // Store the file
    /// let stored_cid = file.store().await?;
    ///
    /// // Load the file
    /// let loaded_file = File::load(&stored_cid, store).await?;
    ///
    /// // Now the initial load CID is set
    /// assert_eq!(loaded_file.get_initial_load_cid(), Some(&stored_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        self.inner.initial_load_cid.get()
    }

    /// Returns the CID of the previous version of the file if there is one.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut file = File::new(store.clone());
    ///
    /// // Initially, there's no previous version
    /// assert!(file.get_previous().is_none());
    ///
    /// // Checkpoint the file multiple times
    /// let v1_cid = file.checkpoint().await?;
    /// let _ = file.checkpoint().await?;
    ///
    /// // Now the previous CID is set
    /// assert_eq!(file.get_previous(), Some(&v1_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_previous(&self) -> Option<&Cid> {
        self.inner.previous.as_ref()
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
    /// let file = File::with_content(store, b"Hello, World!".as_slice()).await?;
    ///
    /// assert!(file.get_content().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_content(&self) -> Option<&Cid> {
        self.inner.content.as_ref()
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
    pub fn get_metadata(&self) -> &Metadata<S> {
        &self.inner.metadata
    }

    /// Returns a mutable reference to the metadata for the file.
    pub fn get_metadata_mut(&mut self) -> &mut Metadata<S> {
        let inner = Arc::make_mut(&mut self.inner);
        &mut inner.metadata
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

    /// Creates a checkpoint of the current file state.
    ///
    /// This is equivalent to storing the file and loading it back,
    /// which is a common pattern when working with versioned files.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::File;
    /// use monoutils_store::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut file = File::with_content(store.clone(), b"Hello, World!".as_slice()).await?;
    ///
    /// // Store and checkpoint the file
    /// let cid = file.checkpoint().await?;
    ///
    /// assert_eq!(file.get_initial_load_cid(), Some(&cid));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn checkpoint(&mut self) -> FsResult<Cid>
    where
        S: Send + Sync,
    {
        let cid = self.store().await?;
        let store = self.inner.store.clone();
        let loaded = Self::load(&cid, store).await?;
        self.inner = loaded.inner;
        Ok(cid)
    }

    /// Returns the size of the file in bytes.
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
    /// let file = File::with_content(store, b"Hello, World!".as_slice()).await?;
    ///
    /// assert_eq!(file.get_size().await?, 13);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_size(&self) -> FsResult<u64> {
        if let Some(cid) = self.get_content() {
            Ok(self.get_store().get_bytes_size(cid).await?)
        } else {
            Ok(0)
        }
    }

    /// Returns `true` if the file is empty.
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
    /// let file = File::new(store);
    ///
    /// assert!(file.is_empty().await?);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub async fn is_empty(&self) -> FsResult<bool> {
        Ok(self.get_size().await? == 0)
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
    /// let mut file = File::with_content(store.clone(), b"Hello, World!".as_slice()).await?;
    ///
    /// assert!(!file.is_empty().await?);
    ///
    /// file.truncate();
    ///
    /// assert!(file.is_empty().await?);
    /// assert!(file.get_content().is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn truncate(&mut self) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.content = None;
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub fn from_serializable(
        serializable: FileSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        let metadata = Metadata::from_serializable(serializable.metadata, store.clone())?;

        Ok(File {
            inner: Arc::new(FileInner {
                initial_load_cid: OnceLock::from(load_cid),
                previous: serializable.previous,
                metadata,
                content: serializable.content,
                store,
            }),
        })
    }

    /// Returns a serializable representation of the file.
    pub async fn get_serializable(&self) -> FsResult<FileSerializable>
    where
        S: Send + Sync,
    {
        let metadata = self.get_metadata().get_serializable().await?;
        Ok(FileSerializable {
            r#type: FILE_TYPE_TAG.to_string(),
            metadata,
            content: self.inner.content,
            previous: self.inner.initial_load_cid.get().cloned(),
        })
    }

    pub(crate) fn set_content(&mut self, content: Option<Cid>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.content = content;
        inner.metadata.set_modified_at(Utc::now());
    }

    pub(crate) fn set_previous(&mut self, previous: Option<Cid>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.previous = previous;
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
        let serializable = self.get_serializable().await.map_err(StoreError::custom)?;
        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        File::from_serializable(serializable, store, *cid).map_err(StoreError::custom)
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
            .field("previous", &self.inner.previous)
            .finish()
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
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::{MemoryStore, Storable};
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn test_file_new() -> anyhow::Result<()> {
        let file = File::new(MemoryStore::default());

        assert!(file.is_empty().await?);
        assert_eq!(file.get_metadata().get_entity_type(), &EntityType::File);
        assert!(file.get_content().is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_file_with_content() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let file = File::with_content(store.clone(), b"Hello, World!".as_slice()).await?;
        assert!(!file.is_empty().await?);

        let content_cid = file.get_content().unwrap();
        let mut content = Vec::new();
        store
            .get_bytes(&content_cid)
            .await?
            .read_to_end(&mut content)
            .await?;

        assert_eq!(content, b"Hello, World!");

        Ok(())
    }

    #[tokio::test]
    async fn test_file_set_content() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());
        assert_eq!(file.get_size().await?, 0);

        // Store some example bytes and get the CID
        let content_cid = store.put_bytes(b"Hello, World!".as_slice()).await?;
        file.set_content(Some(content_cid));

        assert!(!file.is_empty().await?);
        assert_eq!(file.get_content(), Some(&content_cid));
        assert_eq!(file.get_size().await?, 13);
        Ok(())
    }

    #[tokio::test]
    async fn test_file_truncate() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());
        assert_eq!(file.get_size().await?, 0);

        // Store some example bytes and get the CID
        let content_cid = store.put_bytes(b"Hello, World!".as_slice()).await?;

        file.set_content(Some(content_cid));
        assert!(!file.is_empty().await?);
        assert_eq!(file.get_size().await?, 13);

        file.truncate();
        assert!(file.is_empty().await?);
        assert!(file.get_content().is_none());
        assert_eq!(file.get_size().await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_store_and_load() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());

        let content_cid = Cid::default();
        file.set_content(Some(content_cid));

        let stored_cid = file.store().await?;
        let loaded_file = File::load(&stored_cid, store).await?;

        assert_eq!(file.get_content(), loaded_file.get_content());
        assert_eq!(
            file.get_metadata().get_serializable().await.unwrap(),
            loaded_file.get_metadata().get_serializable().await.unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_file_get_initial_load_cid() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let file = File::new(store.clone());

        // Initially, the CID is not set
        assert!(file.get_initial_load_cid().is_none());

        // Store the file
        let stored_cid = file.store().await?;

        // Load the file
        let loaded_file = File::load(&stored_cid, store).await?;

        // Now the initial load CID is set
        assert_eq!(loaded_file.get_initial_load_cid(), Some(&stored_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_file_get_previous() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let file = File::new(store.clone());

        // Initially, there's no previous version
        assert!(file.get_previous().is_none());

        // Store the file
        let first_cid = file.store().await?;

        // Load the file and create a new version
        let mut loaded_file = File::load(&first_cid, store.clone()).await?;
        loaded_file.set_content(Some(Cid::default()));

        // Store the new version
        let second_cid = loaded_file.store().await?;

        // Load the new version
        let new_version = File::load(&second_cid, store).await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(new_version.get_previous(), Some(&first_cid));
        assert_eq!(new_version.get_initial_load_cid(), Some(&second_cid));

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use io::*;

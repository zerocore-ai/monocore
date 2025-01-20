//! Symbolic link implementation.

use std::{
    fmt::{self, Debug},
    str::FromStr,
    sync::{Arc, OnceLock},
};

use chrono::Utc;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};
use typed_path::Utf8UnixPathBuf;

use crate::filesystem::{FsResult, Metadata, MetadataSerializable};

use super::kind::EntityType;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The type identifier for path-based symbolic links.
pub const SYMPATHLINK_TYPE_TAG: &str = "monofs.sympathlink";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A path-based symbolic link that refers to files or directories using relative paths, similar to Unix symlinks.
///
/// ## Path-based Symlinks
///
/// `SymPathLink` is a Unix-like symbolic link that stores a target path relative to its location.
/// The target path is stored as-is and no resolution or validation is performed by monofs, as the
/// target may be beyond monofs's control.
///
/// [symlink]: https://en.wikipedia.org/wiki/Symbolic_link
#[derive(Clone)]
pub struct SymPathLink<S>
where
    S: IpldStore,
{
    inner: Arc<SymPathLinkInner<S>>,
}

#[derive(Clone)]
struct SymPathLinkInner<S>
where
    S: IpldStore,
{
    /// The CID of the symlink when it is initially loaded from the store.
    ///
    /// It is not initialized if the symlink was not loaded from the store.
    initial_load_cid: OnceLock<Cid>,

    /// The CID of the previous version of the symlink.
    previous: Option<Cid>,

    /// The metadata of the symlink.
    metadata: Metadata<S>,

    /// The store of the symlink.
    store: S,

    /// The path that this symlink points to, relative to its location.
    target_path: Utf8UnixPathBuf,
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

/// A serializable representation of a [`SymPathLink`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymPathLinkSerializable {
    /// The type of the entity.
    pub r#type: String,

    /// The metadata of the symlink.
    metadata: MetadataSerializable,

    /// The path of the target of the symlink.
    target: String,

    /// The CID of the previous version of the symlink if there is one.
    previous: Option<Cid>,
}

//--------------------------------------------------------------------------------------------------
// Methods: SymPathLink
//--------------------------------------------------------------------------------------------------

impl<S> SymPathLink<S>
where
    S: IpldStore,
{
    /// Creates a new symlink with the given target path.
    pub fn with_path(store: S, target_path: impl AsRef<str>) -> FsResult<Self> {
        Ok(Self {
            inner: Arc::new(SymPathLinkInner {
                initial_load_cid: OnceLock::new(),
                metadata: Metadata::new(EntityType::SymPathLink, store.clone()),
                store,
                target_path: Utf8UnixPathBuf::from(target_path.as_ref()),
                previous: None,
            }),
        })
    }

    /// Returns the CID of the symlink when it was initially loaded from the store.
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        self.inner.initial_load_cid.get()
    }

    /// Returns the CID of the previous version of the symlink if there is one.
    pub fn get_previous(&self) -> Option<&Cid> {
        self.inner.previous.as_ref()
    }

    /// Returns the metadata for the symlink.
    pub fn get_metadata(&self) -> &Metadata<S> {
        &self.inner.metadata
    }

    /// Returns a mutable reference to the metadata for the symlink.
    pub fn get_metadata_mut(&mut self) -> &mut Metadata<S> {
        let inner = Arc::make_mut(&mut self.inner);
        &mut inner.metadata
    }

    /// Returns the store used to persist the symlink.
    pub fn get_store(&self) -> &S {
        &self.inner.store
    }

    /// Creates a checkpoint of the current symlink state.
    ///
    /// This is equivalent to storing the symlink and loading it back,
    /// which is a common pattern when working with versioned symlinks.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::SymPathLink;
    /// use monoutils_store::{MemoryStore, Storable};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut symlink = SymPathLink::with_path(store.clone(), "test_file.txt")?;
    ///
    /// // Store and checkpoint the symlink
    /// let cid = symlink.checkpoint().await?;
    ///
    /// assert_eq!(symlink.get_initial_load_cid(), Some(&cid));
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

    /// Gets the target path that this symlink points to.
    pub fn get_target_path(&self) -> &Utf8UnixPathBuf {
        &self.inner.target_path
    }

    /// Sets the target path that this symlink points to.
    pub fn set_target_path(&mut self, target_path: impl AsRef<str>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.target_path = Utf8UnixPathBuf::from(target_path.as_ref());
        inner.metadata.set_modified_at(Utc::now());
    }

    /// Tries to create a new `SymPathLink` from a serializable representation.
    pub fn from_serializable(
        serializable: SymPathLinkSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        let metadata = Metadata::from_serializable(serializable.metadata, store.clone())?;
        let target_path = Utf8UnixPathBuf::from_str(&serializable.target)?;

        Ok(SymPathLink {
            inner: Arc::new(SymPathLinkInner {
                initial_load_cid: OnceLock::from(load_cid),
                previous: serializable.previous,
                metadata,
                target_path,
                store,
            }),
        })
    }

    /// Gets the serializable representation of the symlink.
    pub async fn get_serializable(&self) -> FsResult<SymPathLinkSerializable>
    where
        S: Send + Sync,
    {
        let metadata = self.get_metadata().get_serializable().await?;
        Ok(SymPathLinkSerializable {
            r#type: SYMPATHLINK_TYPE_TAG.to_string(),
            metadata,
            target: self.get_target_path().to_string(),
            previous: self.inner.initial_load_cid.get().cloned(),
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

impl<S> Storable<S> for SymPathLink<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let serializable = self.get_serializable().await.map_err(StoreError::custom)?;
        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        SymPathLink::from_serializable(serializable, store, *cid).map_err(StoreError::custom)
    }
}

impl<S> Debug for SymPathLink<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymPathLink")
            .field("metadata", &self.inner.metadata)
            .field("target_path", &self.inner.target_path)
            .field("previous", &self.inner.previous)
            .finish()
    }
}

impl IpldReferences for SymPathLinkSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(std::iter::empty())
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use monoutils_store::MemoryStore;

    #[tokio::test]
    async fn test_sympathlink_creation() -> FsResult<()> {
        let store = MemoryStore::default();
        let symlink = SymPathLink::with_path(store, "test_file.txt")?;

        assert_eq!(
            symlink.get_metadata().get_entity_type(),
            &EntityType::SymPathLink
        );
        assert_eq!(symlink.get_target_path().as_str(), "test_file.txt");

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_store_and_load() -> FsResult<()> {
        let store = MemoryStore::default();
        let symlink = SymPathLink::with_path(store.clone(), "test_file.txt")?;

        // Store the symlink
        let stored_cid = symlink.store().await?;

        // Load the symlink
        let loaded_symlink = SymPathLink::load(&stored_cid, store).await?;

        // Verify the loaded symlink has the same target path
        assert_eq!(loaded_symlink.get_target_path().as_str(), "test_file.txt");
        assert_eq!(loaded_symlink.get_initial_load_cid(), Some(&stored_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_get_previous() -> FsResult<()> {
        let store = MemoryStore::default();
        let symlink = SymPathLink::with_path(store.clone(), "test_file.txt")?;

        // Initially, there's no previous version
        assert!(symlink.get_previous().is_none());

        // Store the symlink
        let first_cid = symlink.store().await?;

        // Load the symlink and create a new version
        let mut loaded_symlink = SymPathLink::load(&first_cid, store.clone()).await?;
        loaded_symlink.set_target_path("new_path.txt");

        // Store the new version
        let second_cid = loaded_symlink.store().await?;

        // Load the new version
        let new_version = SymPathLink::load(&second_cid, store).await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(new_version.get_previous(), Some(&first_cid));
        assert_eq!(new_version.get_initial_load_cid(), Some(&second_cid));
        assert_eq!(new_version.get_target_path().as_str(), "new_path.txt");

        Ok(())
    }
}

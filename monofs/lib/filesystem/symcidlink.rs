//! Symbolic link implementation.

use std::{
    fmt::{self, Debug},
    sync::{Arc, OnceLock},
};

use async_recursion::async_recursion;
use chrono::Utc;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};

use crate::filesystem::{CidLink, Dir, EntityCidLink, File, FsError, FsResult, Metadata};

use super::{entity::Entity, kind::EntityType, MetadataSerializable};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The type identifier for CID-based symbolic links.
pub const SYMCIDLINK_TYPE_TAG: &str = "monofs.symcidlink";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A content-addressed symbolic link that refers to files or directories by their CID (content hash), making it resilient to target moves.
///
/// ## CID-based Symlinks
///
/// Unlike traditional location-addressable file systems where symlinks break if the target entity is moved
/// from its original location, `SymCidLink` refers to an entity by its Cid. This means it only breaks if
/// the Cid to the target entity is deleted, not when the target entity is moved. This can be particularly
/// useful in situations where you want to maintain references to specific versions of entities.
///
/// Note: For Unix-like system compatibility, see [`SymPathLink`](super::sympathlink::SymPathLink).
#[derive(Clone)]
pub struct SymCidLink<S>
where
    S: IpldStore,
{
    inner: Arc<SymCidLinkInner<S>>,
}

#[derive(Clone)]
struct SymCidLinkInner<S>
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

    /// The link to some target [`Entity`].
    link: EntityCidLink<S>,
}

/// Represents the result of following a symlink.
pub enum CidFollowResult<'a, S>
where
    S: IpldStore,
{
    /// The symlink was successfully resolved to a non-symlink entity.
    Resolved(&'a Entity<S>),

    /// The maximum follow depth was reached without resolving to a non-symlink entity.
    MaxDepthReached,

    /// A broken link was encountered during resolution.
    BrokenLink(Cid),
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

/// A serializable representation of [`SymCidLink`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymCidLinkSerializable {
    /// The type of the entity.
    pub r#type: String,

    /// The metadata of the symlink.
    metadata: MetadataSerializable,

    /// The CID of the target of the symlink.
    target: Cid,

    /// The CID of the previous version of the symlink if there is one.
    previous: Option<Cid>,
}

//--------------------------------------------------------------------------------------------------
// Methods: SymCidLink
//--------------------------------------------------------------------------------------------------

impl<S> SymCidLink<S>
where
    S: IpldStore,
{
    /// Creates a new symlink.
    pub fn with_cid(store: S, target: Cid) -> Self {
        Self {
            inner: Arc::new(SymCidLinkInner {
                initial_load_cid: OnceLock::new(),
                metadata: Metadata::new(EntityType::SymCidLink, store.clone()),
                store,
                link: CidLink::from(target),
                previous: None,
            }),
        }
    }

    /// Returns the CID of the symlink when it was initially loaded from the store.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::SymCidLink;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let target_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    /// let symlink = SymCidLink::with_cid(store.clone(), target_cid);
    ///
    /// // Initially, the CID is not set
    /// assert!(symlink.get_initial_load_cid().is_none());
    ///
    /// // Store the symlink
    /// let stored_cid = symlink.store().await?;
    ///
    /// // Load the symlink
    /// let loaded_symlink = SymCidLink::load(&stored_cid, store).await?;
    ///
    /// // Now the initial load CID is set
    /// assert_eq!(loaded_symlink.get_initial_load_cid(), Some(&stored_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        self.inner.initial_load_cid.get()
    }

    /// Returns the CID of the previous version of the symlink if there is one.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::SymCidLink;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let target_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    /// let symlink = SymCidLink::with_cid(store.clone(), target_cid);
    ///
    /// // Initially, there's no previous version
    /// assert!(symlink.get_previous().is_none());
    ///
    /// // Store the symlink
    /// let first_cid = symlink.store().await?;
    ///
    /// // Load the symlink and create a new version
    /// let mut loaded_symlink = SymCidLink::load(&first_cid, store.clone()).await?;
    /// let new_target_cid: Cid = "bafkreihogico5an3e2xy3fykalfwxxry7itbhfcgq6f47sif6d7w6uk2ze".parse()?;
    /// loaded_symlink.set_cid(new_target_cid);
    ///
    /// // Store the new version
    /// let second_cid = loaded_symlink.store().await?;
    ///
    /// // Load the new version
    /// let new_version = SymCidLink::load(&second_cid, store).await?;
    ///
    /// // Now the previous CID is set
    /// assert_eq!(new_version.get_previous(), Some(&first_cid));
    /// # Ok(())
    /// # }
    /// ```
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
    /// use monofs::filesystem::SymCidLink;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let target_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    /// let mut symlink = SymCidLink::with_cid(store.clone(), target_cid);
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

    /// Gets the [`EntityCidLink`] of the target of the symlink.
    pub fn get_link(&self) -> &EntityCidLink<S> {
        &self.inner.link
    }

    /// Sets the [`EntityCidLink`] of the target of the symlink.
    pub fn set_link(&mut self, link: EntityCidLink<S>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.link = link;
        inner.metadata.set_modified_at(Utc::now());
    }

    /// Sets the CID of the target of the symlink.
    pub fn set_cid(&mut self, cid: Cid) {
        self.set_link(CidLink::from(cid));
    }

    /// Sets the [`Dir`] as the target of the symlink.
    pub fn set_dir(&mut self, dir: Dir<S>) {
        self.set_link(EntityCidLink::from(dir));
    }

    /// Sets the [`File`] as the target of the symlink.
    pub fn set_file(&mut self, file: File<S>) {
        self.set_link(EntityCidLink::from(file));
    }

    /// Sets the [`SymCidLink`] as the target of the symlink.
    pub fn set_symlink(&mut self, symlink: SymCidLink<S>) {
        self.set_link(EntityCidLink::from(symlink));
    }

    /// Gets the [`Cid`] of the target of the symlink.
    pub async fn get_cid(&self) -> FsResult<Cid>
    where
        S: Send + Sync,
    {
        self.inner.link.resolve_cid().await
    }

    /// Gets the [`Entity`] that the symlink points to.
    pub async fn get_entity(&self) -> FsResult<&Entity<S>>
    where
        S: Send + Sync,
    {
        self.inner
            .link
            .resolve_entity(self.inner.store.clone())
            .await
    }

    /// Gets the [`Dir`] that the symlink points to.
    pub async fn get_dir(&self) -> FsResult<Option<&Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity().await? {
            Entity::Dir(dir) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] that the symlink points to.
    pub async fn get_file(&self) -> FsResult<Option<&File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity().await? {
            Entity::File(file) => Ok(Some(file)),
            _ => Ok(None),
        }
    }
    /// Gets the [`SymCidLink`] that the symlink points to.
    pub async fn get_symlink(&self) -> FsResult<Option<&SymCidLink<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity().await? {
            Entity::SymCidLink(symlink) => Ok(Some(symlink)),
            _ => Ok(None),
        }
    }

    /// Follows the symlink to resolve the target entity.
    ///
    /// This method will follow the chain of symlinks up to the maximum depth specified in the metadata.
    /// If the maximum depth is reached without resolving to a non-symlink entity, it returns `MaxDepthReached`.
    /// If a broken link is encountered, it returns `BrokenLink`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{SymCidLink, Dir, File, Entity, CidFollowResult};
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    ///
    /// // Create a file
    /// let file = File::with_content(store.clone(), b"Hello, World!".to_vec()).await;
    /// let file_cid = file.store().await?;
    ///
    /// // Create a symlink to the file
    /// let symlink = SymCidLink::with_cid(store.clone(), file_cid);
    ///
    /// // Follow the symlink
    /// match symlink.follow().await? {
    ///     CidFollowResult::Resolved(entity) => {
    ///         assert!(matches!(entity, Entity::File(_)));
    ///     },
    ///     _ => panic!("Expected Resolved, got something else"),
    /// }
    ///
    /// // Create a chain of symlinks
    /// let symlink1 = SymCidLink::with_cid(store.clone(), file_cid);
    /// let symlink1_cid = symlink1.store().await?;
    /// let symlink2 = SymCidLink::with_cid(store.clone(), symlink1_cid);
    ///
    /// // Follow the chain of symlinks
    /// assert!(matches!(symlink2.follow().await?, CidFollowResult::Resolved(Entity::File(_))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn follow(&self) -> FsResult<CidFollowResult<'_, S>>
    where
        S: Send + Sync,
    {
        let max_depth = *self.inner.metadata.get_symlink_depth();
        self.follow_recursive(max_depth).await
    }

    #[async_recursion]
    async fn follow_recursive(
        &self,
        remaining_depth: u32,
    ) -> FsResult<CidFollowResult<'life_self, S>>
    where
        S: Send + Sync,
    {
        if remaining_depth == 0 {
            return Ok(CidFollowResult::MaxDepthReached);
        }

        match self.get_entity().await {
            Ok(entity) => match entity {
                Entity::SymCidLink(next_symlink) => {
                    next_symlink.follow_recursive(remaining_depth - 1).await
                }
                _ => Ok(CidFollowResult::Resolved(entity)),
            },
            Err(FsError::IpldStore(StoreError::BlockNotFound(cid))) => {
                return Ok(CidFollowResult::BrokenLink(cid));
            }
            Err(FsError::IpldStore(StoreError::Custom(any_err))) => {
                if let Some(FsError::UnableToLoadEntity(cid)) = any_err.downcast::<FsError>() {
                    return Ok(CidFollowResult::BrokenLink(*cid));
                }
                Err(StoreError::custom(any_err).into())
            }
            Err(e) => Err(e),
        }
    }

    /// Resolves the symlink to its target entity.
    ///
    /// This method will follow the chain of symlinks up to the maximum depth specified in the metadata.
    /// It will return an error if the maximum depth is reached or if a broken link is encountered.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{SymCidLink, Dir, File, Entity, FsError};
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    ///
    /// // Create a file
    /// let file = File::with_content(store.clone(), b"Hello, World!".to_vec()).await;
    /// let file_cid = file.store().await?;
    ///
    /// // Create a symlink to the file
    /// let symlink = SymCidLink::with_cid(store.clone(), file_cid);
    ///
    /// // Resolve the symlink
    /// let resolved_entity = symlink.resolve().await?;
    /// assert!(matches!(resolved_entity, Entity::File(_)));
    ///
    /// // Create a broken symlink
    /// let non_existent_cid = Cid::default();
    /// let broken_symlink = SymCidLink::with_cid(store.clone(), non_existent_cid);
    ///
    /// // Try to resolve the broken symlink
    /// assert!(matches!(broken_symlink.resolve().await, Err(FsError::BrokenSymCidLink(_))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resolve(&self) -> FsResult<&Entity<S>>
    where
        S: Send + Sync,
    {
        match self.follow().await? {
            CidFollowResult::Resolved(entity) => Ok(entity),
            CidFollowResult::MaxDepthReached => Err(FsError::MaxFollowDepthReached),
            CidFollowResult::BrokenLink(cid) => Err(FsError::BrokenSymCidLink(cid)),
        }
    }

    /// Tries to create a new `SymCidLink` from a serializable representation.
    pub fn from_serializable(
        serializable: SymCidLinkSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        let metadata = Metadata::from_serializable(serializable.metadata, store.clone())?;

        Ok(SymCidLink {
            inner: Arc::new(SymCidLinkInner {
                initial_load_cid: OnceLock::from(load_cid),
                previous: serializable.previous,
                metadata,
                link: CidLink::from(serializable.target),
                store,
            }),
        })
    }

    /// Gets the serializable representation of the symlink.
    pub async fn get_serializable(&self) -> FsResult<SymCidLinkSerializable>
    where
        S: Send + Sync,
    {
        let metadata = self.get_metadata().get_serializable().await?;
        Ok(SymCidLinkSerializable {
            r#type: SYMCIDLINK_TYPE_TAG.to_string(),
            metadata,
            target: self.get_cid().await?,
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

impl<S> From<Entity<S>> for SymCidLink<S>
where
    S: IpldStore + Clone,
{
    fn from(entity: Entity<S>) -> Self {
        Self {
            inner: Arc::new(SymCidLinkInner {
                initial_load_cid: if let Some(cid) = entity.get_initial_load_cid().cloned() {
                    OnceLock::from(cid)
                } else {
                    OnceLock::new()
                },
                metadata: Metadata::new(EntityType::SymCidLink, entity.get_store().clone()),
                store: entity.get_store().clone(),
                link: EntityCidLink::from(entity),
                previous: None,
            }),
        }
    }
}

impl<S> Storable<S> for SymCidLink<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let serializable = self.get_serializable().await.map_err(StoreError::custom)?;
        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        SymCidLink::from_serializable(serializable, store, *cid).map_err(StoreError::custom)
    }
}

impl<S> Debug for SymCidLink<S>
where
    S: IpldStore + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymCidLink")
            .field("metadata", &self.inner.metadata)
            .field("target", &self.inner.link)
            .field("previous", &self.get_previous())
            .finish()
    }
}

impl IpldReferences for SymCidLinkSerializable {
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
    use crate::{
        config::DEFAULT_SYMLINK_DEPTH,
        filesystem::{Dir, Entity, File},
    };
    use monoutils_store::MemoryStore;

    mod fixtures {
        use super::*;

        pub async fn setup_test_env() -> FsResult<(MemoryStore, Dir<MemoryStore>, File<MemoryStore>)>
        {
            let store = MemoryStore::default();
            let mut root_dir = Dir::new(store.clone());

            let file_content = b"Hello, World!".to_vec();
            let file = File::with_content(store.clone(), file_content).await;

            root_dir
                .put_adapted_file("test_file.txt", file.clone())
                .await?;

            Ok((store, root_dir, file))
        }
    }

    #[tokio::test]
    async fn test_symcidlink_creation() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await?;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let symlink = SymCidLink::with_cid(store, file_cid);

        assert_eq!(
            symlink.get_metadata().get_entity_type(),
            &EntityType::SymCidLink
        );
        assert_eq!(symlink.get_cid().await?, file_cid);

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_from_entity() -> FsResult<()> {
        let (_, _, file) = fixtures::setup_test_env().await?;

        let file_entity = Entity::File(file);
        let symlink = SymCidLink::from(file_entity);

        assert_eq!(
            symlink.get_metadata().get_entity_type(),
            &EntityType::SymCidLink
        );
        assert!(matches!(symlink.get_entity().await?, Entity::File(_)));

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_follow() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await?;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let symlink = SymCidLink::with_cid(store, file_cid);

        match symlink.follow().await? {
            CidFollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_to_symcidlink() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await?;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let symlink1 = SymCidLink::with_cid(store.clone(), file_cid);

        let symlink1_cid = symlink1.store().await?;
        let symlink2 = SymCidLink::with_cid(store, symlink1_cid);

        match symlink2.follow().await? {
            CidFollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_max_depth() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await?;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;

        // Link depth 1 to file.
        let mut symlink = SymCidLink::with_cid(store.clone(), file_cid);

        // Link depth 9 to file.
        for _ in 0..DEFAULT_SYMLINK_DEPTH - 1 {
            let cid = symlink.store().await?;
            symlink = SymCidLink::with_cid(store.clone(), cid);
        }

        match symlink.follow().await? {
            CidFollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        // Link depth 10 to file.
        let cid = symlink.store().await?;
        symlink = SymCidLink::with_cid(store.clone(), cid);

        match symlink.follow().await? {
            CidFollowResult::MaxDepthReached => {}
            _ => panic!("Expected MaxDepthReached, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_broken() -> FsResult<()> {
        let store = MemoryStore::default();
        let non_existent_cid = Cid::default(); // This CID doesn't exist in the store

        let symlink = SymCidLink::with_cid(store, non_existent_cid);

        match symlink.follow().await? {
            CidFollowResult::BrokenLink(_) => {}
            _ => panic!("Expected BrokenLink, got something else"),
        }

        assert!(matches!(
            symlink.resolve().await,
            Err(FsError::BrokenSymCidLink(_))
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_resolve() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await?;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let symlink = SymCidLink::with_cid(store, file_cid);

        let resolved_entity = symlink.resolve().await?;
        assert!(matches!(resolved_entity, Entity::File(_)));

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_get_initial_load_cid() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let target_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        let symlink = SymCidLink::with_cid(store.clone(), target_cid);

        // Initially, the CID is not set
        assert!(symlink.get_initial_load_cid().is_none());

        // Store the symlink
        let stored_cid = symlink.store().await?;

        // Load the symlink
        let loaded_symlink = SymCidLink::load(&stored_cid, store).await?;

        // Now the initial load CID is set
        assert_eq!(loaded_symlink.get_initial_load_cid(), Some(&stored_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_symcidlink_get_previous() -> FsResult<()> {
        let store = MemoryStore::default();
        let target_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        let symlink = SymCidLink::with_cid(store.clone(), target_cid);

        // Initially, there's no previous version
        assert!(symlink.get_previous().is_none());

        // Store the symlink
        let first_cid = symlink.store().await?;

        // Load the symlink and create a new version
        let mut loaded_symlink = SymCidLink::load(&first_cid, store.clone()).await?;

        let new_target_cid: Cid =
            "bafkreihogico5an3e2xy3fykalfwxxry7itbhfcgq6f47sif6d7w6uk2ze".parse()?;
        loaded_symlink.set_cid(new_target_cid);

        // Store the new version
        let second_cid = loaded_symlink.store().await?;

        // Load the new version
        let new_version = SymCidLink::load(&second_cid, store).await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(new_version.get_previous(), Some(&first_cid));
        assert_eq!(new_version.get_initial_load_cid(), Some(&second_cid));

        Ok(())
    }
}

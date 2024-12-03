//! Symbolic link implementation.

use std::{
    fmt::{self, Debug},
    sync::{Arc, OnceLock},
};

use async_recursion::async_recursion;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};

use crate::filesystem::{CidLink, Dir, EntityCidLink, File, FsError, FsResult, Metadata};

use super::{entity::Entity, kind::EntityType};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a [`symbolic link`][softlink] to a file or directory in the `monofs` _immutable_ file system.
///
/// ## Important
///
/// Entities in `monofs` are designed to be immutable and clone-on-write meaning writes create
/// forks of the entity.
///
/// [softlink]: https://en.wikipedia.org/wiki/Symbolic_link
#[derive(Clone)]
pub struct SoftLink<S>
where
    S: IpldStore,
{
    inner: Arc<SoftLinkInner<S>>,
}

#[derive(Clone)]
struct SoftLinkInner<S>
where
    S: IpldStore,
{
    /// The CID of the softlink when it is initially loaded from the store.
    ///
    /// It is not initialized if the softlink was not loaded from the store.
    initial_load_cid: OnceLock<Cid>,

    /// The CID of the previous version of the softlink.
    previous: Option<Cid>,

    /// The metadata of the softlink.
    metadata: Metadata,

    /// The store of the softlink.
    store: S,

    /// The (weak) link to some target [`Entity`].
    // TODO: Because `SoftLink` refers to an entity by its Cid, it's behavior is a bit different from
    // typical location-addressable file systems where softlinks break if the target entity is moved
    // from its original location. `SoftLink` only breaks if the Cid to the target entity is deleted
    // not the target entity itself. This is bad.
    //
    // In order to maintain compatibility with Unix-like systems, we may need to change this to an
    // `EntityPathLink<S>` in the future, where the path is relative to the location of the softlink.
    link: EntityCidLink<S>,
}

/// Represents the result of following a softlink.
pub enum FollowResult<'a, S>
where
    S: IpldStore,
{
    /// The softlink was successfully resolved to a non-softlink entity.
    Resolved(&'a Entity<S>),

    /// The maximum follow depth was reached without resolving to a non-softlink entity.
    MaxDepthReached,

    /// A broken link was encountered during resolution.
    BrokenLink(Cid),
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SoftLinkSerializable {
    metadata: Metadata,
    target: Cid,
    previous: Option<Cid>,
}

//--------------------------------------------------------------------------------------------------
// Methods: SoftLink
//--------------------------------------------------------------------------------------------------

impl<S> SoftLink<S>
where
    S: IpldStore,
{
    /// Creates a new softlink.
    pub fn with_cid(store: S, target: Cid) -> Self {
        Self {
            inner: Arc::new(SoftLinkInner {
                initial_load_cid: OnceLock::new(),
                metadata: Metadata::new(EntityType::SoftLink),
                store,
                link: CidLink::from(target),
                previous: None,
            }),
        }
    }

    /// Returns the CID of the softlink when it was initially loaded from the store.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::SoftLink;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let target_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    /// let softlink = SoftLink::with_cid(store.clone(), target_cid);
    ///
    /// // Initially, the CID is not set
    /// assert!(softlink.get_initial_load_cid().is_none());
    ///
    /// // Store the softlink
    /// let stored_cid = softlink.store().await?;
    ///
    /// // Load the softlink
    /// let loaded_softlink = SoftLink::load(&stored_cid, store).await?;
    ///
    /// // Now the initial load CID is set
    /// assert_eq!(loaded_softlink.get_initial_load_cid(), Some(&stored_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_initial_load_cid(&self) -> Option<&Cid> {
        self.inner.initial_load_cid.get()
    }

    /// Returns the CID of the previous version of the softlink if there is one.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::SoftLink;
    /// use monoutils_store::{MemoryStore, Storable};
    /// use monoutils_store::ipld::cid::Cid;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let target_cid: Cid = "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;
    /// let softlink = SoftLink::with_cid(store.clone(), target_cid);
    ///
    /// // Initially, there's no previous version
    /// assert!(softlink.get_previous().is_none());
    ///
    /// // Store the softlink
    /// let first_cid = softlink.store().await?;
    ///
    /// // Load the softlink and create a new version
    /// let mut loaded_softlink = SoftLink::load(&first_cid, store.clone()).await?;
    /// let new_target_cid: Cid = "bafkreihogico5an3e2xy3fykalfwxxry7itbhfcgq6f47sif6d7w6uk2ze".parse()?;
    /// loaded_softlink.set_cid(new_target_cid);
    ///
    /// // Store the new version
    /// let second_cid = loaded_softlink.store().await?;
    ///
    /// // Load the new version
    /// let new_version = SoftLink::load(&second_cid, store).await?;
    ///
    /// // Now the previous CID is set
    /// assert_eq!(new_version.get_previous(), Some(&first_cid));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_previous(&self) -> Option<&Cid> {
        self.inner.previous.as_ref()
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata {
        &self.inner.metadata
    }

    /// Returns the store used to persist the softlink.
    pub fn get_store(&self) -> &S {
        &self.inner.store
    }

    /// Gets the [`EntityCidLink`] of the target of the softlink.
    pub fn get_link(&self) -> &EntityCidLink<S> {
        &self.inner.link
    }

    /// Sets the [`EntityCidLink`] of the target of the softlink.
    pub fn set_link(&mut self, link: EntityCidLink<S>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.link = link;
    }

    /// Sets the CID of the target of the softlink.
    pub fn set_cid(&mut self, cid: Cid) {
        self.set_link(CidLink::from(cid));
    }

    /// Sets the [`Dir`] as the target of the softlink.
    pub fn set_dir(&mut self, dir: Dir<S>) {
        self.set_link(EntityCidLink::from(dir));
    }

    /// Sets the [`File`] as the target of the softlink.
    pub fn set_file(&mut self, file: File<S>) {
        self.set_link(EntityCidLink::from(file));
    }

    /// Sets the [`SoftLink`] as the target of the softlink.
    pub fn set_softlink(&mut self, softlink: SoftLink<S>) {
        self.set_link(EntityCidLink::from(softlink));
    }

    /// Gets the [`Cid`] of the target of the softlink.
    pub async fn get_cid(&self) -> FsResult<Cid>
    where
        S: Send + Sync,
    {
        self.inner.link.resolve_cid().await
    }

    /// Gets the [`Entity`] that the softlink points to.
    pub async fn get_entity(&self) -> FsResult<&Entity<S>>
    where
        S: Send + Sync,
    {
        self.inner
            .link
            .resolve_entity(self.inner.store.clone())
            .await
    }

    /// Gets the [`Dir`] that the softlink points to.
    pub async fn get_dir(&self) -> FsResult<Option<&Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity().await? {
            Entity::Dir(dir) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] that the softlink points to.
    pub async fn get_file(&self) -> FsResult<Option<&File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity().await? {
            Entity::File(file) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`SoftLink`] that the softlink points to.
    pub async fn get_softlink(&self) -> FsResult<Option<&SoftLink<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity().await? {
            Entity::SoftLink(softlink) => Ok(Some(softlink)),
            _ => Ok(None),
        }
    }

    /// Follows the softlink to resolve the target entity.
    ///
    /// This method will follow the chain of softlinks up to the maximum depth specified in the metadata.
    /// If the maximum depth is reached without resolving to a non-softlink entity, it returns `MaxDepthReached`.
    /// If a broken link is encountered, it returns `BrokenLink`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{SoftLink, Dir, File, Entity, FollowResult};
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
    /// // Create a softlink to the file
    /// let softlink = SoftLink::with_cid(store.clone(), file_cid);
    ///
    /// // Follow the softlink
    /// match softlink.follow().await? {
    ///     FollowResult::Resolved(entity) => {
    ///         assert!(matches!(entity, Entity::File(_)));
    ///     },
    ///     _ => panic!("Expected Resolved, got something else"),
    /// }
    ///
    /// // Create a chain of softlinks
    /// let softlink1 = SoftLink::with_cid(store.clone(), file_cid);
    /// let softlink1_cid = softlink1.store().await?;
    /// let softlink2 = SoftLink::with_cid(store.clone(), softlink1_cid);
    ///
    /// // Follow the chain of softlinks
    /// assert!(matches!(softlink2.follow().await?, FollowResult::Resolved(Entity::File(_))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn follow(&self) -> FsResult<FollowResult<'_, S>>
    where
        S: Send + Sync,
    {
        let max_depth = *self.inner.metadata.get_softlink_depth();
        self.follow_recursive(max_depth).await
    }

    #[async_recursion]
    async fn follow_recursive(&self, remaining_depth: u32) -> FsResult<FollowResult<'life_self, S>>
    where
        S: Send + Sync,
    {
        if remaining_depth == 0 {
            return Ok(FollowResult::MaxDepthReached);
        }

        match self.get_entity().await {
            Ok(entity) => match entity {
                Entity::SoftLink(next_softlink) => {
                    next_softlink.follow_recursive(remaining_depth - 1).await
                }
                _ => Ok(FollowResult::Resolved(entity)),
            },
            // We find the error `get_entity` returns that deals with not being able to load an entity
            // from the store and return a `FollowResult::BrokenLink`.
            Err(FsError::IpldStore(StoreError::Custom(any_err))) => {
                if let Some(FsError::UnableToLoadEntity(cid)) = any_err.downcast::<FsError>() {
                    return Ok(FollowResult::BrokenLink(*cid));
                }

                return Err(StoreError::custom(any_err).into());
            }
            Err(e) => Err(e),
        }
    }

    /// Resolves the softlink to its target entity.
    ///
    /// This method will follow the chain of softlinks up to the maximum depth specified in the metadata.
    /// It will return an error if the maximum depth is reached or if a broken link is encountered.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{SoftLink, Dir, File, Entity, FsError};
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
    /// // Create a softlink to the file
    /// let softlink = SoftLink::with_cid(store.clone(), file_cid);
    ///
    /// // Resolve the softlink
    /// let resolved_entity = softlink.resolve().await?;
    /// assert!(matches!(resolved_entity, Entity::File(_)));
    ///
    /// // Create a broken softlink
    /// let non_existent_cid = Cid::default();
    /// let broken_softlink = SoftLink::with_cid(store.clone(), non_existent_cid);
    ///
    /// // Try to resolve the broken softlink
    /// assert!(matches!(broken_softlink.resolve().await, Err(FsError::BrokenSoftLink(_))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resolve(&self) -> FsResult<&Entity<S>>
    where
        S: Send + Sync,
    {
        match self.follow().await? {
            FollowResult::Resolved(entity) => Ok(entity),
            FollowResult::MaxDepthReached => Err(FsError::MaxFollowDepthReached),
            FollowResult::BrokenLink(cid) => Err(FsError::BrokenSoftLink(cid)),
        }
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub(crate) fn from_serializable(
        serializable: SoftLinkSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        Ok(SoftLink {
            inner: Arc::new(SoftLinkInner {
                initial_load_cid: OnceLock::from(load_cid),
                previous: serializable.previous,
                metadata: serializable.metadata,
                link: CidLink::from(serializable.target),
                store,
            }),
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> From<Entity<S>> for SoftLink<S>
where
    S: IpldStore + Clone,
{
    fn from(entity: Entity<S>) -> Self {
        Self {
            inner: Arc::new(SoftLinkInner {
                initial_load_cid: if let Some(cid) = entity.get_initial_load_cid().cloned() {
                    OnceLock::from(cid)
                } else {
                    OnceLock::new()
                },
                metadata: Metadata::new(EntityType::SoftLink),
                store: entity.get_store().clone(),
                link: EntityCidLink::from(entity),
                previous: None,
            }),
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

impl<S> From<SoftLink<S>> for Entity<S>
where
    S: IpldStore + Clone,
{
    fn from(softlink: SoftLink<S>) -> Self {
        Entity::SoftLink(softlink)
    }
}

impl<S> Storable<S> for SoftLink<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let serializable = SoftLinkSerializable {
            metadata: self.inner.metadata.clone(),
            target: self
                .inner
                .link
                .resolve_cid()
                .await
                .map_err(StoreError::custom)?,
            previous: self.inner.initial_load_cid.get().cloned(),
        };

        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        SoftLink::from_serializable(serializable, store, *cid).map_err(StoreError::custom)
    }
}

impl<S> Debug for SoftLink<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SoftLink")
            .field("metadata", &self.inner.metadata)
            .finish()
    }
}

impl IpldReferences for SoftLinkSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        // This empty because `SoftLink`s cannot have strong references to other entities.
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
        config::DEFAULT_SOFTLINK_DEPTH,
        filesystem::{Dir, Entity, File},
    };
    use monoutils_store::MemoryStore;

    mod fixtures {
        use super::*;

        pub async fn setup_test_env() -> (MemoryStore, Dir<MemoryStore>, File<MemoryStore>) {
            let store = MemoryStore::default();
            let mut root_dir = Dir::new(store.clone());

            let file_content = b"Hello, World!".to_vec();
            let file = File::with_content(store.clone(), file_content).await;

            root_dir.put_file("test_file.txt", file.clone()).unwrap();

            (store, root_dir, file)
        }
    }

    #[tokio::test]
    async fn test_softlink_creation() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let softlink = SoftLink::with_cid(store, file_cid);

        assert_eq!(
            softlink.get_metadata().get_entity_type(),
            &EntityType::SoftLink
        );
        assert_eq!(softlink.get_cid().await?, file_cid);

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_from_entity() -> FsResult<()> {
        let (_, _, file) = fixtures::setup_test_env().await;

        let file_entity = Entity::File(file);
        let softlink = SoftLink::from(file_entity);

        assert_eq!(
            softlink.get_metadata().get_entity_type(),
            &EntityType::SoftLink
        );
        assert!(matches!(softlink.get_entity().await?, Entity::File(_)));

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_follow() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let softlink = SoftLink::with_cid(store, file_cid);

        match softlink.follow().await? {
            FollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_to_softlink() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let softlink1 = SoftLink::with_cid(store.clone(), file_cid);

        let softlink1_cid = softlink1.store().await?;
        let softlink2 = SoftLink::with_cid(store, softlink1_cid);

        match softlink2.follow().await? {
            FollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_max_depth() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;

        // Link depth 1 to file.
        let mut softlink = SoftLink::with_cid(store.clone(), file_cid);

        // Link depth 9 to file.
        for _ in 0..DEFAULT_SOFTLINK_DEPTH - 1 {
            let cid = softlink.store().await?;
            softlink = SoftLink::with_cid(store.clone(), cid);
        }

        match softlink.follow().await? {
            FollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        // Link depth 10 to file.
        let cid = softlink.store().await?;
        softlink = SoftLink::with_cid(store.clone(), cid);

        match softlink.follow().await? {
            FollowResult::MaxDepthReached => {}
            _ => panic!("Expected MaxDepthReached, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_broken() -> FsResult<()> {
        let store = MemoryStore::default();
        let non_existent_cid = Cid::default(); // This CID doesn't exist in the store

        let softlink = SoftLink::with_cid(store, non_existent_cid);

        match softlink.follow().await? {
            FollowResult::BrokenLink(_) => {}
            _ => panic!("Expected BrokenLink, got something else"),
        }

        assert!(matches!(
            softlink.resolve().await,
            Err(FsError::BrokenSoftLink(_))
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_resolve() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let file_cid = root_dir
            .get_entry("test_file.txt")?
            .unwrap()
            .resolve_cid()
            .await?;
        let softlink = SoftLink::with_cid(store, file_cid);

        let resolved_entity = softlink.resolve().await?;
        assert!(matches!(resolved_entity, Entity::File(_)));

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_get_initial_load_cid() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let target_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        let softlink = SoftLink::with_cid(store.clone(), target_cid);

        // Initially, the CID is not set
        assert!(softlink.get_initial_load_cid().is_none());

        // Store the softlink
        let stored_cid = softlink.store().await?;

        // Load the softlink
        let loaded_softlink = SoftLink::load(&stored_cid, store).await?;

        // Now the initial load CID is set
        assert_eq!(loaded_softlink.get_initial_load_cid(), Some(&stored_cid));

        Ok(())
    }

    #[tokio::test]
    async fn test_softlink_get_previous() -> FsResult<()> {
        let store = MemoryStore::default();
        let target_cid: Cid =
            "bafkreidgvpkjawlxz6sffxzwgooowe5yt7i6wsyg236mfoks77nywkptdq".parse()?;

        let softlink = SoftLink::with_cid(store.clone(), target_cid);

        // Initially, there's no previous version
        assert!(softlink.get_previous().is_none());

        // Store the softlink
        let first_cid = softlink.store().await?;

        // Load the softlink and create a new version
        let mut loaded_softlink = SoftLink::load(&first_cid, store.clone()).await?;

        let new_target_cid: Cid =
            "bafkreihogico5an3e2xy3fykalfwxxry7itbhfcgq6f47sif6d7w6uk2ze".parse()?;
        loaded_softlink.set_cid(new_target_cid);

        // Store the new version
        let second_cid = loaded_softlink.store().await?;

        // Load the new version
        let new_version = SoftLink::load(&second_cid, store).await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(new_version.get_previous(), Some(&first_cid));
        assert_eq!(new_version.get_initial_load_cid(), Some(&second_cid));

        Ok(())
    }
}

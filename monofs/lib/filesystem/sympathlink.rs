use std::{
    fmt::{self, Debug},
    sync::{Arc, OnceLock},
};

use async_recursion::async_recursion;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};
use typed_path::Utf8UnixPathBuf;

use crate::filesystem::{Dir, Entity, EntityPathLink, File, FsError, FsResult, Metadata};

use super::{kind::EntityType, MetadataSerializable};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A path-based symbolic link that refers to files or directories using relative paths, similar to Unix symlinks.
///
/// ## Important
///
/// Entities in `monofs` are designed to be immutable and clone-on-write meaning writes create
/// forks of the entity.
///
/// ## Path-based Symlinks
///
/// Unlike CID-based symlinks which refer to entities by their content hash, `SymPathLink` refers to
/// entities by their path relative to the symlink's location. This provides Unix-like system compatibility
/// where symlinks can break if the target entity is moved from its original location.
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

    /// The link to some target [`Entity`].
    link: EntityPathLink<S>,
}

/// Represents the result of following a symlink.
pub enum PathFollowResult<'a, S>
where
    S: IpldStore,
{
    /// The symlink was successfully resolved to a non-symlink entity.
    Resolved(&'a Entity<S>),

    /// The maximum follow depth was reached without resolving to a non-symlink entity.
    MaxDepthReached,

    /// A broken link was encountered during resolution.
    BrokenLink(String),
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SymPathLinkSerializable {
    metadata: MetadataSerializable,
    target: String,
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
    pub fn with_path(store: S, target: impl AsRef<str>) -> FsResult<Self> {
        Ok(Self {
            inner: Arc::new(SymPathLinkInner {
                initial_load_cid: OnceLock::new(),
                metadata: Metadata::new(EntityType::SymPathLink, store.clone()),
                store,
                link: EntityPathLink::from(Utf8UnixPathBuf::from(target.as_ref())),
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

    /// Gets the [`EntityPathLink`] of the target of the symlink.
    pub fn get_link(&self) -> &EntityPathLink<S> {
        &self.inner.link
    }

    /// Sets the [`EntityPathLink`] of the target of the symlink.
    pub fn set_link(&mut self, link: EntityPathLink<S>) {
        let inner = Arc::make_mut(&mut self.inner);
        inner.link = link;
    }

    /// Gets the [`Entity`] that the symlink points to.
    ///
    /// The `parent_dir` parameter is the directory containing this symlink, which is used
    /// to resolve the target path relative to the symlink's location.
    pub async fn get_entity<'a>(&'a self, parent_dir: &'a Dir<S>) -> FsResult<&'a Entity<S>>
    where
        S: Send + Sync,
    {
        self.inner.link.resolve_entity(parent_dir).await
    }

    /// Gets the [`Dir`] that the symlink points to.
    ///
    /// The `parent_dir` parameter is the directory containing this symlink, which is used
    /// to resolve the target path relative to the symlink's location.
    pub async fn get_dir<'a>(&'a self, parent_dir: &'a Dir<S>) -> FsResult<Option<&'a Dir<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(parent_dir).await? {
            Entity::Dir(dir) => Ok(Some(dir)),
            _ => Ok(None),
        }
    }

    /// Gets the [`File`] that the symlink points to.
    ///
    /// The `parent_dir` parameter is the directory containing this symlink, which is used
    /// to resolve the target path relative to the symlink's location.
    pub async fn get_file<'a>(&'a self, parent_dir: &'a Dir<S>) -> FsResult<Option<&'a File<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(parent_dir).await? {
            Entity::File(file) => Ok(Some(file)),
            _ => Ok(None),
        }
    }

    /// Gets the [`SymPathLink`] that the symlink points to.
    ///
    /// The `parent_dir` parameter is the directory containing this symlink, which is used
    /// to resolve the target path relative to the symlink's location.
    pub async fn get_symlink<'a>(
        &'a self,
        parent_dir: &'a Dir<S>,
    ) -> FsResult<Option<&'a SymPathLink<S>>>
    where
        S: Send + Sync,
    {
        match self.get_entity(parent_dir).await? {
            Entity::SymPathLink(symlink) => Ok(Some(symlink)),
            _ => Ok(None),
        }
    }

    /// Follows the symlink to resolve the target entity.
    ///
    /// This method will follow the chain of symlinks up to the maximum depth specified in the metadata.
    /// If the maximum depth is reached without resolving to a non-symlink entity, it returns `MaxDepthReached`.
    /// If a broken link is encountered, it returns `BrokenLink`.
    ///
    /// The `parent_dir` parameter is the directory containing this symlink, which is used
    /// to resolve the target path relative to the symlink's location.
    pub async fn follow<'a>(&'a self, parent_dir: &'a Dir<S>) -> FsResult<PathFollowResult<'a, S>>
    where
        S: Send + Sync,
    {
        let max_depth = *self.inner.metadata.get_symlink_depth();
        self.follow_recursive(parent_dir, max_depth).await
    }

    #[async_recursion]
    async fn follow_recursive<'a>(
        &'a self,
        parent_dir: &'a Dir<S>,
        remaining_depth: u32,
    ) -> FsResult<PathFollowResult<'a, S>>
    where
        S: Send + Sync,
    {
        if remaining_depth == 0 {
            return Ok(PathFollowResult::MaxDepthReached);
        }

        match self.get_entity(parent_dir).await {
            Ok(entity) => match entity {
                Entity::SymPathLink(next_symlink) => {
                    next_symlink
                        .follow_recursive(parent_dir, remaining_depth - 1)
                        .await
                }
                _ => Ok(PathFollowResult::Resolved(entity)),
            },
            Err(FsError::PathNotFound(path)) => Ok(PathFollowResult::BrokenLink(path)),
            Err(e) => Err(e),
        }
    }

    /// Resolves the symlink to its target entity.
    ///
    /// This method will follow the chain of symlinks up to the maximum depth specified in the metadata.
    /// It will return an error if the maximum depth is reached or if a broken link is encountered.
    ///
    /// The `parent_dir` parameter is the directory containing this symlink, which is used
    /// to resolve the target path relative to the symlink's location.
    pub async fn resolve<'a>(&'a self, parent_dir: &'a Dir<S>) -> FsResult<&'a Entity<S>>
    where
        S: Send + Sync,
    {
        match self.follow(parent_dir).await? {
            PathFollowResult::Resolved(entity) => Ok(entity),
            PathFollowResult::MaxDepthReached => Err(FsError::MaxFollowDepthReached),
            PathFollowResult::BrokenLink(path) => Err(FsError::PathNotFound(path)),
        }
    }

    /// Tries to create a new `SymPathLink` from a serializable representation.
    pub(crate) fn from_serializable(
        serializable: SymPathLinkSerializable,
        store: S,
        load_cid: Cid,
    ) -> FsResult<Self> {
        let metadata = Metadata::from_serializable(serializable.metadata, store.clone())?;
        let link = serializable.target.try_into()?;

        Ok(SymPathLink {
            inner: Arc::new(SymPathLinkInner {
                initial_load_cid: OnceLock::from(load_cid),
                previous: serializable.previous,
                metadata,
                link,
                store,
            }),
        })
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
        let metadata = self
            .inner
            .metadata
            .get_serializable()
            .await
            .map_err(StoreError::custom)?;

        let serializable = SymPathLinkSerializable {
            metadata,
            target: self.inner.link.get_path().to_string(),
            previous: self.inner.initial_load_cid.get().cloned(),
        };

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
    use crate::{
        config::DEFAULT_SYMLINK_DEPTH,
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
    async fn test_sympathlink_creation() -> FsResult<()> {
        let store = MemoryStore::default();
        let symlink = SymPathLink::with_path(store, "test_file.txt")?;

        assert_eq!(
            symlink.get_metadata().get_entity_type(),
            &EntityType::SymPathLink
        );
        assert_eq!(symlink.get_link().get_path().as_str(), "test_file.txt");

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_follow() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let symlink = SymPathLink::with_path(store, "test_file.txt")?;

        match symlink.follow(&root_dir).await? {
            PathFollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_to_sympathlink() -> FsResult<()> {
        let (store, mut root_dir, _) = fixtures::setup_test_env().await;

        // Create first symlink pointing to test_file.txt
        let symlink1 = SymPathLink::with_path(store.clone(), "test_file.txt")?;
        root_dir.put_entity("link1", Entity::SymPathLink(symlink1.clone()))?;

        // Create second symlink pointing to link1
        let symlink2 = SymPathLink::with_path(store, "link1")?;

        match symlink2.follow(&root_dir).await? {
            PathFollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_max_depth() -> FsResult<()> {
        let (store, mut root_dir, _) = fixtures::setup_test_env().await;

        // Create initial symlink pointing to test_file.txt
        let mut current_name = String::from("test_file.txt");
        let mut current_symlink = SymPathLink::with_path(store.clone(), &current_name)?;

        // Create chain of symlinks up to DEFAULT_SYMLINK_DEPTH - 1
        for i in 0..DEFAULT_SYMLINK_DEPTH - 1 {
            let name = format!("link{}", i);
            root_dir.put_entity(&name, Entity::SymPathLink(current_symlink.clone()))?;
            current_name = name;
            current_symlink = SymPathLink::with_path(store.clone(), &current_name)?;
        }

        // This should still resolve
        match current_symlink.follow(&root_dir).await? {
            PathFollowResult::Resolved(entity) => {
                assert!(matches!(entity, Entity::File(_)));
            }
            _ => panic!("Expected Resolved, got something else"),
        }

        // Add one more level, exceeding max depth
        let name = format!("link{}", DEFAULT_SYMLINK_DEPTH);
        root_dir.put_entity(&name, Entity::SymPathLink(current_symlink.clone()))?;
        let final_symlink = SymPathLink::with_path(store, &name)?;

        match final_symlink.follow(&root_dir).await? {
            PathFollowResult::MaxDepthReached => {}
            _ => panic!("Expected MaxDepthReached, got something else"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_broken() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let symlink = SymPathLink::with_path(store, "non_existent.txt")?;

        match symlink.follow(&root_dir).await? {
            PathFollowResult::BrokenLink(_) => {}
            _ => panic!("Expected BrokenLink, got something else"),
        }

        assert!(matches!(
            symlink.resolve(&root_dir).await,
            Err(FsError::PathNotFound(_))
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_resolve() -> FsResult<()> {
        let (store, root_dir, _) = fixtures::setup_test_env().await;

        let symlink = SymPathLink::with_path(store, "test_file.txt")?;

        let resolved_entity = symlink.resolve(&root_dir).await?;
        assert!(matches!(resolved_entity, Entity::File(_)));

        Ok(())
    }

    #[tokio::test]
    async fn test_sympathlink_get_initial_load_cid() -> FsResult<()> {
        let store = MemoryStore::default();
        let symlink = SymPathLink::with_path(store.clone(), "test_file.txt")?;

        // Initially, the CID is not set
        assert!(symlink.get_initial_load_cid().is_none());

        // Store the symlink
        let stored_cid = symlink.store().await?;

        // Load the symlink
        let loaded_symlink = SymPathLink::load(&stored_cid, store).await?;

        // Now the initial load CID is set
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
        loaded_symlink.set_link(EntityPathLink::from(Utf8UnixPathBuf::from("new_path.txt")));

        // Store the new version
        let second_cid = loaded_symlink.store().await?;

        // Load the new version
        let new_version = SymPathLink::load(&second_cid, store).await?;

        // Now the previous and initial load CIDs are set
        assert_eq!(new_version.get_previous(), Some(&first_cid));
        assert_eq!(new_version.get_initial_load_cid(), Some(&second_cid));

        Ok(())
    }
}

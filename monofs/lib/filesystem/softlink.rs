//! Symbolic link implementation.

use std::{
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

use crate::{dir::Dir, file::File, CidLink, EntityCidLink, FsError, FsResult, Metadata};

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
    /// The metadata of the softlink.
    pub(crate) metadata: Metadata,

    /// The store of the softlink.
    pub(crate) store: S,

    /// The link to the target of the softlink.
    ///
    /// ## Note
    ///
    /// Because `SoftLink` refers to an entity by its Cid, it's behavior is a bit different from
    /// typical location-addressable file systems where softlinks break if the target entity is moved
    /// from its original location.
    pub(crate) link: EntityCidLink<S>,
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SoftLinkSerializable {
    metadata: Metadata,
    target: Cid,
}

pub(crate) struct SoftLinkDeserializeSeed<S> {
    pub(crate) store: S,
}

//--------------------------------------------------------------------------------------------------
// Methods: SoftLink
//--------------------------------------------------------------------------------------------------

impl<S> SoftLink<S>
where
    S: IpldStore,
{
    /// Creates a new softlink.
    pub fn new(store: S, target: Cid) -> Self {
        Self {
            inner: Arc::new(SoftLinkInner {
                metadata: Metadata::new(EntityType::SoftLink),
                store,
                link: CidLink::from(target),
            }),
        }
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata {
        &self.inner.metadata
    }

    /// Gets the [`Cid`] of the target of the softlink.
    pub async fn get_cid(&self) -> FsResult<Cid>
    where
        S: Send + Sync,
    {
        self.inner.link.resolve_cid().await
    }

    /// Gets the [`EntityCidLink`] of the target of the softlink.
    pub fn get_link(&self) -> &EntityCidLink<S> {
        &self.inner.link
    }

    /// Gets the [`Entity`] that the softlink points to.
    pub async fn get_entity(&self) -> FsResult<&Entity<S>>
    where
        S: Send + Sync,
    {
        self.inner
            .link
            .resolve_value(self.inner.store.clone())
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

    /// Deserializes to a `Dir` using an arbitrary deserializer and store.
    pub fn deserialize_with<'de>(
        deserializer: impl Deserializer<'de, Error: Into<FsError>>,
        store: S,
    ) -> FsResult<Self> {
        SoftLinkDeserializeSeed::new(store)
            .deserialize(deserializer)
            .map_err(Into::into)
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub(crate) fn try_from_serializable(
        serializable: SoftLinkSerializable,
        store: S,
    ) -> FsResult<Self> {
        Ok(SoftLink {
            inner: Arc::new(SoftLinkInner {
                metadata: serializable.metadata,
                link: CidLink::from(serializable.target),
                store,
            }),
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Methods: FileDeserializeSeed
//--------------------------------------------------------------------------------------------------

impl<S> SoftLinkDeserializeSeed<S> {
    fn new(store: S) -> Self {
        Self { store }
    }
}
//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<'de, S> DeserializeSeed<'de> for SoftLinkDeserializeSeed<S>
where
    S: IpldStore,
{
    type Value = SoftLink<S>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let serializable = SoftLinkSerializable::deserialize(deserializer)?;
        SoftLink::try_from_serializable(serializable, self.store).map_err(de::Error::custom)
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
        };

        self.inner.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        SoftLink::try_from_serializable(serializable, store).map_err(StoreError::custom)
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

// impl<S> PartialEq for SoftLink<S>
// where
//     S: IpldStore,
// {
//     fn eq(&self, other: &Self) -> bool {
//         self.inner.metadata == other.inner.metadata && self.inner.link == other.inner.link
//     }
// }

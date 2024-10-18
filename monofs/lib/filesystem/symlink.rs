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
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{CidLink, EntityCidLink, FsError, FsResult, Metadata};

use super::kind::EntityType;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a [`symbolic link`][symlink] to a file or directory in the `monofs` file system.
///
/// ## Important
///
/// Entities in `monofs` are designed to be immutable and clone-on-write meaning writes create
/// forks of the entity.
///
/// [symlink]: https://en.wikipedia.org/wiki/Symbolic_link
#[derive(Clone)]
pub struct Symlink<S>
where
    S: IpldStore,
{
    inner: Arc<SymlinkInner<S>>,
}

#[derive(Clone)]
struct SymlinkInner<S>
where
    S: IpldStore,
{
    /// The metadata of the symlink.
    pub(crate) metadata: Metadata,

    /// The store of the symlink.
    pub(crate) store: S,

    /// The link to the target of the symlink.
    ///
    /// ## Note
    ///
    /// Because `SymLink` refers to an entity by its Cid, it's behavior is a bit different from
    /// typical location-addressable file systems where symlinks break if the target entity is moved
    /// from its original location.
    pub(crate) link: EntityCidLink<S>,
}

//--------------------------------------------------------------------------------------------------
// Types: Serializable
//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SymlinkSerializable {
    metadata: Metadata,
    target: Cid,
}

pub(crate) struct SymlinkDeserializeSeed<S> {
    pub(crate) store: S,
}

//--------------------------------------------------------------------------------------------------
// Methods: Symlink
//--------------------------------------------------------------------------------------------------

impl<S> Symlink<S>
where
    S: IpldStore,
{
    /// Creates a new symlink.
    pub fn new(store: S, target: Cid) -> Self {
        Self {
            inner: Arc::new(SymlinkInner {
                metadata: Metadata::new(EntityType::Symlink),
                store,
                link: CidLink::from(target),
            }),
        }
    }

    /// Returns the metadata for the directory.
    pub fn get_metadata(&self) -> &Metadata {
        &self.inner.metadata
    }

    /// Gets the target of the symlink.
    pub fn get_target(&self) -> &Cid {
        self.inner.link.get_cid()
    }

    /// Change the store used to persist the symlink.
    pub fn use_store<T>(self, store: T) -> Symlink<T>
    where
        T: IpldStore,
    {
        let inner = match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner,
            Err(arc) => (*arc).clone(),
        };

        Symlink {
            inner: Arc::new(SymlinkInner {
                metadata: inner.metadata,
                link: inner.link.use_store(&store),
                store,
            }),
        }
    }

    /// Deserializes to a `Dir` using an arbitrary deserializer and store.
    pub fn deserialize_with<'de>(
        deserializer: impl Deserializer<'de, Error: Into<FsError>>,
        store: S,
    ) -> FsResult<Self> {
        SymlinkDeserializeSeed::new(store)
            .deserialize(deserializer)
            .map_err(Into::into)
    }

    /// Tries to create a new `Dir` from a serializable representation.
    pub(crate) fn try_from_serializable(
        serializable: SymlinkSerializable,
        store: S,
    ) -> FsResult<Self> {
        Ok(Symlink {
            inner: Arc::new(SymlinkInner {
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

impl<S> SymlinkDeserializeSeed<S> {
    fn new(store: S) -> Self {
        Self { store }
    }
}
//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> IpldReferences for Symlink<S>
where
    S: IpldStore,
{
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(std::iter::empty())
    }
}

impl<S> Serialize for Symlink<S>
where
    S: IpldStore,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        let serializable = SymlinkSerializable {
            metadata: self.inner.metadata.clone(),
            target: *self.inner.link.get_cid(),
        };

        serializable.serialize(serializer)
    }
}

impl<'de, S> DeserializeSeed<'de> for SymlinkDeserializeSeed<S>
where
    S: IpldStore,
{
    type Value = Symlink<S>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let serializable = SymlinkSerializable::deserialize(deserializer)?;
        Symlink::try_from_serializable(serializable, self.store).map_err(de::Error::custom)
    }
}

impl<S> Storable<S> for Symlink<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        self.inner.store.put_node(self).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable = store.get_node(cid).await?;
        Symlink::try_from_serializable(serializable, store).map_err(StoreError::custom)
    }
}

impl<S> Debug for Symlink<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Symlink")
            .field("metadata", &self.inner.metadata)
            .finish()
    }
}

impl<S> PartialEq for Symlink<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner.metadata == other.inner.metadata && self.inner.link == other.inner.link
    }
}

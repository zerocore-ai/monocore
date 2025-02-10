use std::collections::BTreeMap;
use std::fmt::{self, Debug};
use std::iter;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use getset::Getters;
use monoutils_store::{
    ipld::{cid::Cid, ipld::Ipld},
    IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::FsResult;

use super::{kind::EntityType, AttributesCidLink};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Key for storing Unix file mode in extended attributes.
pub const UNIX_MODE_KEY: &str = "unix.mode";

/// Key for storing Unix user ID in extended attributes.
pub const UNIX_UID_KEY: &str = "unix.uid";

/// Key for storing Unix group ID in extended attributes.
pub const UNIX_GID_KEY: &str = "unix.gid";

/// Key for storing Unix access time in extended attributes.
pub const UNIX_ATIME_KEY: &str = "unix.atime";

/// Key for storing Unix modification time in extended attributes.
pub const UNIX_MTIME_KEY: &str = "unix.mtime";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Relevant metadata for a file system entity.
///
/// This mostly corresponds to the `fd-stat` in POSIX. `monofs` does not support
/// hard links, so there is no `link-count` field. Also `size` is not stored here, but rather
/// requested when needed.
///
/// ## Examples
///
/// ```
/// use monofs::filesystem::{EntityType, Metadata, SyncType};
/// use monofs::config::DEFAULT_SYMLINK_DEPTH;
/// use monoutils_store::MemoryStore;
///
/// let store = MemoryStore::default();
/// let metadata = Metadata::new(EntityType::File, store);
///
/// assert_eq!(*metadata.get_entity_type(), EntityType::File);
/// assert_eq!(*metadata.get_sync_type(), SyncType::Default);
/// assert_eq!(*metadata.get_symlink_depth(), DEFAULT_SYMLINK_DEPTH);
/// ```
#[derive(Clone, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Metadata<S>
where
    S: IpldStore,
{
    /// The type of the entity.
    entity_type: EntityType,

    /// The time the entity was created.
    created_at: DateTime<Utc>,

    /// The time of the last modification of the entity.
    modified_at: DateTime<Utc>,

    /// The sync type of the entity.
    sync_type: SyncType,

    /// Extended attributes.
    extended_attrs: Option<AttributesCidLink<S>>,

    /// The store of the metadata.
    store: S,
}

/// The type of sync used for the entity.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SyncType {
    /// Use the configured default method
    #[default]
    Default,

    /// Use the [RAFT consensus algorithm][raft] to sync the entity.
    ///
    /// [raft]: https://raft.github.io/
    Raft,

    /// Use [Merkle-CRDT][merkle-crdt] as the method of syncing.
    ///
    /// [merkle-crdt]: https://research.protocol.ai/publications/merkle-crdts-merkle-dags-meet-crdts/psaras2020.pdf
    MerkleCRDT,
}

/// Extended attributes for a file system entity.
///
/// This struct provides a thread-safe way to store and manage extended attributes (xattrs) for file system entities.
/// Extended attributes are key-value pairs that can be associated with files, directories, and other file system entities
/// to store additional metadata beyond the standard attributes.
///
/// The attributes are stored in a `BTreeMap` wrapped in an `Arc<RwLock>` to allow safe concurrent access and modification.
/// Values are stored as IPLD (InterPlanetary Linked Data) to support a wide range of data types and structures.
///
/// ## Examples
///
/// ```
/// use monofs::filesystem::{EntityType, Metadata};
/// use monoutils_store::MemoryStore;
/// use monoutils_store::ipld::ipld::Ipld;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let store = MemoryStore::default();
/// let mut metadata = Metadata::new(EntityType::File, store);
///
/// // Set extended attributes
/// metadata.set_attribute("user.description", "Important document").await?;
/// metadata.set_attribute("user.tags", vec![Ipld::from("document"), Ipld::from("important")]).await?;
///
/// // Read extended attributes
/// let description = metadata.get_attribute("user.description").await?;
/// let tags = metadata.get_attribute("user.tags").await?;
/// # Ok(())
/// # }
/// ```
pub struct ExtendedAttributes<S> {
    inner: Arc<RwLock<ExtendedAttributesInner<S>>>,
}

#[derive(Debug)]
struct ExtendedAttributesInner<S> {
    /// The map of extended attributes.
    map: BTreeMap<String, Arc<Ipld>>,

    /// The store used to persist the extended attributes.
    store: S,
}

//--------------------------------------------------------------------------------------------------
// Types: *
//--------------------------------------------------------------------------------------------------

/// A serializable representation of [`Metadata`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSerializable {
    entity_type: EntityType,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
    sync_type: SyncType,
    extended_attrs: Option<Cid>,
}

/// A serializable representation of [`ExtendedAttributes`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtendedAttributesSerializable(BTreeMap<String, Ipld>);

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> Metadata<S>
where
    S: IpldStore,
{
    /// Creates a new metadata instance with the given entity type and store.
    pub fn new(entity_type: EntityType, store: S) -> Self {
        let now = Utc::now();
        Self {
            entity_type,
            created_at: now,
            modified_at: now,
            sync_type: SyncType::default(),
            extended_attrs: None,
            store,
        }
    }

    /// Creates a new metadata instance from a serializable representation.
    pub fn from_serializable(serializable: MetadataSerializable, store: S) -> FsResult<Self> {
        Ok(Self {
            entity_type: serializable.entity_type,
            created_at: serializable.created_at,
            modified_at: serializable.modified_at,
            sync_type: serializable.sync_type,
            extended_attrs: serializable
                .extended_attrs
                .map(|cid| AttributesCidLink::from(cid)),
            store,
        })
    }

    /// Gets a serializable representation of the metadata.
    pub async fn get_serializable(&self) -> FsResult<MetadataSerializable>
    where
        S: Send + Sync,
    {
        let extended_attrs = if let Some(attrs) = &self.extended_attrs {
            Some(attrs.resolve_cid().await?)
        } else {
            None
        };

        Ok(MetadataSerializable {
            entity_type: self.entity_type,
            created_at: self.created_at,
            modified_at: self.modified_at,
            sync_type: self.sync_type,
            extended_attrs,
        })
    }

    /// Gets the value of an attribute.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{EntityType, Metadata};
    /// use monoutils_store::MemoryStore;
    /// use monoutils_store::ipld::ipld::Ipld;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut metadata = Metadata::new(EntityType::File, store);
    ///
    /// // Initially no attribute exists
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, None);
    ///
    /// // Set an attribute
    /// metadata.set_attribute("custom.attr", "value").await?;
    ///
    /// // Now we can get the attribute
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, Some(Arc::new(Ipld::String("value".to_string()))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_attribute(&self, key: impl AsRef<str>) -> FsResult<Option<Arc<Ipld>>>
    where
        S: Send + Sync,
    {
        match &self.extended_attrs {
            Some(link) => {
                let attrs = link.resolve_value(self.store.clone()).await?;
                let ipld = attrs.inner.read().await.map.get(key.as_ref()).cloned();
                Ok(ipld)
            }
            None => Ok(None),
        }
    }

    /// Sets the value of an attribute.
    ///
    /// If the extended attributes don't exist, they will be created.
    /// If the attribute already exists, its value will be updated.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{EntityType, Metadata};
    /// use monoutils_store::MemoryStore;
    /// use monoutils_store::ipld::ipld::Ipld;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut metadata = Metadata::new(EntityType::File, store);
    ///
    /// // Set an attribute
    /// metadata.set_attribute("custom.attr", "value").await?;
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, Some(Arc::new(Ipld::String("value".to_string()))));
    ///
    /// // Update an existing attribute
    /// metadata.set_attribute("custom.attr", "new value").await?;
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, Some(Arc::new(Ipld::String("new value".to_string()))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_attribute(
        &mut self,
        key: impl Into<String>,
        value: impl Into<Ipld>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        let key = key.into();
        let value = value.into();

        match &mut self.extended_attrs {
            Some(link) => {
                let attrs = link.resolve_value_mut(self.store.clone()).await?;
                attrs.inner.write().await.map.insert(key, Arc::new(value));
            }
            None => {
                let mut map = BTreeMap::new();
                map.insert(key, Arc::new(value));
                let attrs = ExtendedAttributes {
                    inner: Arc::new(RwLock::new(ExtendedAttributesInner {
                        map,
                        store: self.store.clone(),
                    })),
                };
                self.extended_attrs = Some(AttributesCidLink::from(attrs));
            }
        }
        Ok(())
    }

    /// Sets the sync type.
    pub fn set_sync_type(&mut self, sync_type: SyncType) {
        self.sync_type = sync_type;
    }

    /// Sets the modified timestamp.
    pub fn set_modified_at(&mut self, modified_at: DateTime<Utc>) {
        self.modified_at = modified_at;
    }

    /// Sets the created timestamp.
    pub fn set_created_at(&mut self, created_at: DateTime<Utc>) {
        self.created_at = created_at;
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> Storable<S> for Metadata<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let serializable = self.get_serializable().await.map_err(StoreError::custom)?;
        self.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable: MetadataSerializable = store.get_node(cid).await?;
        Metadata::from_serializable(serializable, store).map_err(StoreError::custom)
    }
}

impl<S> Storable<S> for ExtendedAttributes<S>
where
    S: IpldStore + Send + Sync,
{
    async fn store(&self) -> StoreResult<Cid> {
        let map = self
            .inner
            .read()
            .await
            .map
            .iter()
            .map(|(k, v)| (k.clone(), (**v).clone()))
            .collect::<BTreeMap<_, _>>();

        let serializable = ExtendedAttributesSerializable(map);
        self.inner.write().await.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable: BTreeMap<String, Ipld> = store.get_node(cid).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(ExtendedAttributesInner {
                map: serializable
                    .into_iter()
                    .map(|(k, v)| (k, Arc::new(v)))
                    .collect(),
                store,
            })),
        })
    }
}

impl IpldReferences for MetadataSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(self.extended_attrs.iter())
    }
}

impl IpldReferences for ExtendedAttributesSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(iter::empty())
    }
}

impl<S> Debug for Metadata<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metadata")
            .field("entity_type", &self.entity_type)
            .field("created_at", &self.created_at)
            .field("modified_at", &self.modified_at)
            .field("sync_type", &self.sync_type)
            .field(
                "extended_attrs",
                &self.extended_attrs.as_ref().map(|link| link.get_cid()),
            )
            .finish()
    }
}

// impl<S> ExtendedAttributes<S>
// where
//     S: IpldStore,
// {
//     /// Gets a reference to the map of extended attributes.
//     pub fn get_map(&self) -> &BTreeMap<String, Ipld> {
//         &self.map
//     }
// }

impl<S> Clone for ExtendedAttributes<S>
where
    S: IpldStore,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
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
    fn test_metadata_new() {
        let store = MemoryStore::default();
        let metadata = Metadata::new(EntityType::File, store);

        assert_eq!(*metadata.get_entity_type(), EntityType::File);
        assert_eq!(*metadata.get_sync_type(), SyncType::Default);
    }

    #[test]
    fn test_metadata_getters() {
        let store = MemoryStore::default();
        let metadata = Metadata::new(EntityType::File, store);

        assert_eq!(*metadata.get_entity_type(), EntityType::File);
        assert_eq!(*metadata.get_sync_type(), SyncType::Default);
    }

    #[tokio::test]
    async fn test_metadata_stores_loads() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let metadata = Metadata::new(EntityType::File, store.clone());

        let cid = metadata.store().await?;
        let loaded = Metadata::load(&cid, store).await?;

        assert_eq!(*loaded.get_entity_type(), EntityType::File);
        assert_eq!(*loaded.get_sync_type(), SyncType::Default);

        Ok(())
    }

    #[tokio::test]
    async fn test_metadata_attributes() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut metadata = Metadata::new(EntityType::File, store.clone());

        // Initially no attributes exist
        assert_eq!(metadata.get_attribute("test.attr1").await?, None);
        assert_eq!(metadata.get_attribute("test.attr2").await?, None);

        // Set some attributes
        metadata.set_attribute("test.attr1", "value1").await?;
        metadata.set_attribute("test.attr2", "value2").await?;

        // Verify attributes were set
        assert_eq!(
            metadata.get_attribute("test.attr1").await?,
            Some(Arc::new(Ipld::String("value1".to_string())))
        );
        assert_eq!(
            metadata.get_attribute("test.attr2").await?,
            Some(Arc::new(Ipld::String("value2".to_string())))
        );

        // Update an existing attribute
        metadata.set_attribute("test.attr1", "new value").await?;
        assert_eq!(
            metadata.get_attribute("test.attr1").await?,
            Some(Arc::new(Ipld::String("new value".to_string())))
        );

        // Store and load the metadata to verify persistence
        let cid = metadata.store().await?;
        let loaded_metadata = Metadata::load(&cid, store).await?;

        // Verify attributes persisted
        assert_eq!(
            loaded_metadata.get_attribute("test.attr1").await?,
            Some(Arc::new(Ipld::String("new value".to_string())))
        );
        assert_eq!(
            loaded_metadata.get_attribute("test.attr2").await?,
            Some(Arc::new(Ipld::String("value2".to_string())))
        );

        // Non-existent attribute still returns None
        assert_eq!(loaded_metadata.get_attribute("nonexistent").await?, None);

        Ok(())
    }
}

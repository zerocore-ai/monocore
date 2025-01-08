use std::collections::BTreeMap;
use std::fmt::{self, Debug};
use std::iter;

use chrono::{DateTime, Utc};
use getset::Getters;
use monoutils_store::{
    ipld::cid::Cid, IpldReferences, IpldStore, Storable, StoreError, StoreResult,
};
use serde::{Deserialize, Serialize};

use crate::config::DEFAULT_SYMLINK_DEPTH;

use super::{kind::EntityType, AttributesCidLink, FsResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The key for the created at field in the metadata.
pub const CREATED_AT_KEY: &str = "monofs.created_at";

/// The key for the entity type field in the metadata.
pub const ENTITY_TYPE_KEY: &str = "monofs.entity_type";

/// The key for the modified at field in the metadata.
pub const MODIFIED_AT_KEY: &str = "monofs.modified_at";

/// The key for the symbolic link depth field in the metadata.
pub const SYMLINK_DEPTH_KEY: &str = "monofs.symlink_depth";

/// The key for the sync type field in the metadata.
pub const SYNC_TYPE_KEY: &str = "monofs.sync_type";

/// The key for the tombstone field in the metadata.
pub const TOMBSTONE_KEY: &str = "monofs.tombstone";

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
/// assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
/// assert_eq!(*metadata.get_symlink_depth(), DEFAULT_SYMLINK_DEPTH);
/// ```
#[derive(Clone, Serialize, Deserialize, Getters)]
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

    /// The maximum depth of a symbolic link.
    symlink_depth: u32,

    /// The sync type of the entity.
    sync_type: SyncType,

    /// Whether the entity is a tombstone.
    tombstone: bool,

    /// Extended attributes.
    #[serde(skip)]
    extended_attrs: Option<AttributesCidLink<S>>,

    /// The store of the metadata.
    store: S,
}

/// The type of sync used for the entity.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncType {
    /// Use the [RAFT consensus algorithm][raft] to sync the entity.
    ///
    /// [raft]: https://raft.github.io/
    RAFT,

    /// Use [Merkle-CRDT][merkle-crdt] as the method of syncing.
    ///
    /// [merkle-crdt]: https://research.protocol.ai/publications/merkle-crdts-merkle-dags-meet-crdts/psaras2020.pdf
    MerkleCRDT,
}

/// Extended attributes for a file system entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedAttributes<S> {
    /// The map of extended attributes.
    map: BTreeMap<String, String>,

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
    symlink_depth: u32,
    sync_type: SyncType,
    tombstone: bool,
    extended_attrs: Option<Cid>,
}

/// A serializable representation of [`ExtendedAttributes`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtendedAttributesSerializable<'a>(&'a BTreeMap<String, String>);

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> Metadata<S>
where
    S: IpldStore,
{
    /// Creates a new metadata object.
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
    /// assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
    /// assert_eq!(*metadata.get_symcidlink_depth(), DEFAULT_SYMLINK_DEPTH);
    /// ```
    pub fn new(entity_type: EntityType, store: S) -> Self {
        let now = Utc::now();

        Self {
            entity_type,
            created_at: now,
            modified_at: now,
            symlink_depth: DEFAULT_SYMLINK_DEPTH,
            sync_type: SyncType::RAFT,
            tombstone: false,
            extended_attrs: None,
            store,
        }
    }

    /// Tries to create a new `Metadata` from a serializable representation.
    pub fn from_serializable(serializable: MetadataSerializable, store: S) -> FsResult<Self> {
        Ok(Self {
            entity_type: serializable.entity_type,
            created_at: serializable.created_at,
            modified_at: serializable.modified_at,
            symlink_depth: serializable.symlink_depth,
            sync_type: serializable.sync_type,
            tombstone: serializable.tombstone,
            extended_attrs: serializable
                .extended_attrs
                .map(|cid| AttributesCidLink::from(cid)),
            store,
        })
    }

    /// Gets the serializable representation of the metadata.
    pub async fn get_serializable(&self) -> FsResult<MetadataSerializable>
    where
        S: Send + Sync,
    {
        let extended_attrs = match &self.extended_attrs {
            Some(link) => Some(link.resolve_cid().await?),
            None => None,
        };

        Ok(MetadataSerializable {
            entity_type: self.entity_type,
            created_at: self.created_at,
            modified_at: self.modified_at,
            symlink_depth: self.symlink_depth,
            sync_type: self.sync_type,
            tombstone: self.tombstone,
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
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, Some("value".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_attribute(&self, key: impl AsRef<str>) -> FsResult<Option<String>>
    where
        S: Send + Sync,
    {
        match &self.extended_attrs {
            Some(link) => {
                let attrs = link.resolve_value(self.store.clone()).await?;
                Ok(attrs.map.get(key.as_ref()).cloned())
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
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let store = MemoryStore::default();
    /// let mut metadata = Metadata::new(EntityType::File, store);
    ///
    /// // Set an attribute
    /// metadata.set_attribute("custom.attr", "value").await?;
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, Some("value".to_string()));
    ///
    /// // Update an existing attribute
    /// metadata.set_attribute("custom.attr", "new value").await?;
    /// assert_eq!(metadata.get_attribute("custom.attr").await?, Some("new value".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_attribute(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> FsResult<()>
    where
        S: Send + Sync,
    {
        let key = key.into();
        let value = value.into();

        match &mut self.extended_attrs {
            Some(link) => {
                let attrs = link.resolve_value_mut(self.store.clone()).await?;
                attrs.map.insert(key, value);
            }
            None => {
                let mut map = BTreeMap::new();
                map.insert(key, value);
                let attrs = ExtendedAttributes {
                    map,
                    store: self.store.clone(),
                };
                self.extended_attrs = Some(AttributesCidLink::from(attrs));
            }
        }
        Ok(())
    }

    /// Sets the maximum depth of a symbolic link.
    pub fn set_symlink_depth(&mut self, depth: u32) {
        self.symlink_depth = depth;
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
        let serializable = ExtendedAttributesSerializable(&self.map);
        self.store.put_node(&serializable).await
    }

    async fn load(cid: &Cid, store: S) -> StoreResult<Self> {
        let serializable: BTreeMap<String, String> = store.get_node(cid).await?;
        Ok(Self {
            map: serializable,
            store,
        })
    }
}

impl IpldReferences for MetadataSerializable {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(self.extended_attrs.iter())
    }
}

impl IpldReferences for ExtendedAttributesSerializable<'_> {
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
            .field("symlink_depth", &self.symlink_depth)
            .field("sync_type", &self.sync_type)
            .field("tombstone", &self.tombstone)
            .field(
                "extended_attrs",
                &self.extended_attrs.as_ref().map(|link| link.get_cid()),
            )
            .finish()
    }
}

impl<S> ExtendedAttributes<S>
where
    S: IpldStore,
{
    /// Gets a reference to the map of extended attributes.
    pub fn get_map(&self) -> &BTreeMap<String, String> {
        &self.map
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
        let metadata: Metadata<MemoryStore> = Metadata::new(EntityType::File, store);

        assert_eq!(*metadata.get_entity_type(), EntityType::File);
        assert_eq!(*metadata.get_symlink_depth(), DEFAULT_SYMLINK_DEPTH);
        assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
        assert!(!metadata.get_tombstone());
    }

    #[test]
    fn test_metadata_getters() {
        let store = MemoryStore::default();
        let metadata = Metadata::new(EntityType::Dir, store);

        assert_eq!(*metadata.get_entity_type(), EntityType::Dir);
        assert_eq!(*metadata.get_symlink_depth(), DEFAULT_SYMLINK_DEPTH);
        assert!(metadata.get_created_at() <= &Utc::now());
        assert!(metadata.get_modified_at() <= &Utc::now());
        assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
        assert!(!metadata.get_tombstone());
    }

    #[tokio::test]
    async fn test_metadata_stores_loads() {
        let store = MemoryStore::default();
        let metadata = Metadata::new(EntityType::File, store.clone());

        let cid = metadata.store().await.unwrap();
        let loaded_metadata = Metadata::load(&cid, store).await.unwrap();

        assert_eq!(
            metadata.get_entity_type(),
            loaded_metadata.get_entity_type()
        );
        assert_eq!(
            metadata.get_symlink_depth(),
            loaded_metadata.get_symlink_depth()
        );
        assert_eq!(metadata.get_sync_type(), loaded_metadata.get_sync_type());
        assert_eq!(metadata.get_tombstone(), loaded_metadata.get_tombstone());
        assert_eq!(metadata.get_created_at(), loaded_metadata.get_created_at());
        assert_eq!(
            metadata.get_modified_at(),
            loaded_metadata.get_modified_at()
        );
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
            Some("value1".to_string())
        );
        assert_eq!(
            metadata.get_attribute("test.attr2").await?,
            Some("value2".to_string())
        );

        // Update an existing attribute
        metadata.set_attribute("test.attr1", "new value").await?;
        assert_eq!(
            metadata.get_attribute("test.attr1").await?,
            Some("new value".to_string())
        );

        // Store and load the metadata to verify persistence
        let cid = metadata.store().await?;
        let loaded_metadata = Metadata::load(&cid, store).await?;

        // Verify attributes persisted
        assert_eq!(
            loaded_metadata.get_attribute("test.attr1").await?,
            Some("new value".to_string())
        );
        assert_eq!(
            loaded_metadata.get_attribute("test.attr2").await?,
            Some("value2".to_string())
        );

        // Non-existent attribute still returns None
        assert_eq!(loaded_metadata.get_attribute("nonexistent").await?, None);

        Ok(())
    }
}

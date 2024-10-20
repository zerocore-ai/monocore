use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use getset::Getters;
use serde::{Deserialize, Serialize};

use crate::config::DEFAULT_SOFTLINK_DEPTH;

use super::kind::EntityType;

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
/// use monofs::config::DEFAULT_SOFTLINK_DEPTH;
///
/// let metadata = Metadata::new(EntityType::File);
/// assert_eq!(*metadata.get_entity_type(), EntityType::File);
/// assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
/// assert_eq!(*metadata.get_softlink_depth(), DEFAULT_SOFTLINK_DEPTH);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Metadata {
    /// The type of the entity.
    entity_type: EntityType,

    /// The time the entity was created.
    created_at: DateTime<Utc>,

    /// The time of the last modification of the entity.
    modified_at: DateTime<Utc>,

    /// The size of the entity in bytes.
    sync_type: SyncType,

    /// The maximum depth of a softlink.
    softlink_depth: u32,
    // /// Extended attributes.
    // #[serde(skip)]
    // extended_attrs: Option<AttributeCidLink<S>>,
}

/// The method of syncing to use for the entity used by the filesystem
/// service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SyncType {
    /// Use the [RAFT consensus algorithm][raft] to sync the entity.
    ///
    /// [raft]: https://raft.github.io/
    #[default]
    RAFT,

    /// Use [Merkle-CRDT][crdt] as the method of syncing.
    ///
    /// [crdt]: https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type
    CRDT,
}

/// Extended attributes for a file system entity.
pub struct ExtendedAttributes<S> {
    /// The map of extended attributes.
    _map: BTreeMap<String, String>,

    /// The store used to persist the extended attributes.
    _store: S,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Metadata {
    /// Creates a new metadata object.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{EntityType, Metadata, SyncType};
    /// use monofs::config::DEFAULT_SOFTLINK_DEPTH;
    ///
    /// let metadata = Metadata::new(EntityType::File);
    /// assert_eq!(*metadata.get_entity_type(), EntityType::File);
    /// assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
    /// assert_eq!(*metadata.get_softlink_depth(), DEFAULT_SOFTLINK_DEPTH);
    /// ```
    pub fn new(entity_type: EntityType) -> Self {
        let now = Utc::now();

        Self {
            entity_type,
            created_at: now,
            modified_at: now,
            sync_type: SyncType::default(),
            softlink_depth: DEFAULT_SOFTLINK_DEPTH,
            // extended_attrs: None,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_new() {
        let metadata = Metadata::new(EntityType::File);

        assert_eq!(*metadata.get_entity_type(), EntityType::File);
        assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
        assert_eq!(*metadata.get_softlink_depth(), DEFAULT_SOFTLINK_DEPTH);
    }

    #[test]
    fn test_metadata_getters() {
        let metadata = Metadata::new(EntityType::Dir);

        assert_eq!(*metadata.get_entity_type(), EntityType::Dir);
        assert_eq!(*metadata.get_sync_type(), SyncType::RAFT);
        assert_eq!(*metadata.get_softlink_depth(), DEFAULT_SOFTLINK_DEPTH);
        assert!(metadata.get_created_at() <= &Utc::now());
        assert!(metadata.get_modified_at() <= &Utc::now());
    }

    #[test]
    fn test_sync_type_default() {
        assert_eq!(SyncType::default(), SyncType::RAFT);
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = Metadata::new(EntityType::File);
        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: Metadata = serde_json::from_str(&serialized).unwrap();

        assert_eq!(metadata, deserialized);
    }
}

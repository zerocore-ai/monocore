use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use getset::Getters;
use serde::{Deserialize, Serialize};

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
// TODO: Need to to know precisely what the DateTimes serialize to.
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
    // /// The maximum depth of a softlink.
    // softlink_depth: u32,

    // /// Extended attributes.
    // #[serde(skip)]
    // extended_attrs: Option<AttributeCidLink<S>>,
}

/// The method of syncing to use for the entity used by the filesystem
/// service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    pub fn new(entity_type: EntityType) -> Self {
        let now = Utc::now();

        Self {
            entity_type,
            created_at: now,
            modified_at: now,
            sync_type: SyncType::default(),
            // softlink_depth: 40,
            // extended_attrs: None,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

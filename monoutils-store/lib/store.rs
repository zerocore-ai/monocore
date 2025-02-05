use std::{collections::HashSet, future::Future, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use ipld_core::cid::Cid;
use monoutils::SeekableReader;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};

use super::{IpldReferences, StoreError, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The different codecs supported by the IPLD store.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Codec {
    /// Raw bytes.
    Raw,

    /// DAG-CBOR codec.
    DagCbor,

    /// DAG-JSON codec.
    DagJson,

    /// DAG-PB codec.
    DagPb,

    /// Unknown codec.
    Unknown(u64),
}

//--------------------------------------------------------------------------------------------------
// Traits: IpldStore, IpldStoreSeekable, IpldStoreExt, *
//--------------------------------------------------------------------------------------------------

/// `IpldStore` is a content-addressable store for [`IPLD` (InterPlanetary Linked Data)][ipld] that
/// emphasizes the structured nature of the data it stores.
///
/// It can store raw bytes of data and structured data stored as IPLD. Stored data can be fetched
/// by their [`CID`s (Content Identifier)][cid] which is represents the fingerprint of the data.
///
/// ## Implementation Note
///
/// It is advisable that the type implementing `IpldStore` implements cheap clone semantics (e.g.,
/// using `Arc`) since several operations on `IpldStore` require cloning the store. Using types with
/// expensive clone operations may impact performance.
///
/// [cid]: https://docs.ipfs.tech/concepts/content-addressing/
/// [ipld]: https://ipld.io/
#[async_trait]
pub trait IpldStore: RawStore + Clone {
    /// Stores a serializable object in the store using the appropriate IPLD codec.
    ///
    /// The object must implement both `Serialize` and `IpldReferences`. The object will be serialized
    /// using one of the supported IPLD codecs (see [`get_supported_codecs`]). Any CIDs referenced by
    /// the object (via [`IpldReferences`]) will have their reference counts incremented.
    ///
    /// ## Arguments
    ///
    /// * `node` - The object to store
    ///
    /// ## Returns
    ///
    /// Returns the CID of the stored object.
    ///
    /// ## Errors
    ///
    /// Returns `StoreError::NodeBlockTooLarge` if the serialized data exceeds the store's maximum node block size.
    async fn put_node<T>(&self, node: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync;

    /// Stores raw bytes in the store, automatically chunking them if necessary.
    ///
    /// Large amounts of data are automatically split into smaller blocks according to the store's
    /// chunking strategy. The chunks are then organized using the store's layout strategy, which may
    /// create intermediate merkle nodes to represent the structure.
    ///
    /// ## Arguments
    ///
    /// * `reader` - An async reader providing the bytes to store
    ///
    /// ## Returns
    ///
    /// Returns the CID that can be used to retrieve the entire data.
    ///
    /// ## Errors
    ///
    /// Returns `StoreError::RawBlockTooLarge` if any chunk exceeds the store's maximum block size.
    async fn put_bytes(&self, reader: impl AsyncRead + Send + Sync) -> StoreResult<Cid>;

    /// Retrieves and deserializes an object from the store.
    ///
    /// The object must be deserializable from the IPLD codec that was used to store it.
    ///
    /// ## Arguments
    ///
    /// * `cid` - The CID of the object to retrieve
    ///
    /// ## Returns
    ///
    /// Returns the deserialized object.
    ///
    /// ## Errors
    ///
    /// Returns `StoreError::BlockNotFound` if no block exists with the given CID.
    /// Returns `StoreError::UnexpectedBlockCodec` if the block's codec doesn't match the expected codec.
    async fn get_node<D>(&self, cid: &Cid) -> StoreResult<D>
    where
        D: DeserializeOwned + Send;

    /// Creates an async reader to access the bytes associated with a CID.
    ///
    /// For chunked data, this will automatically handle reading across chunk boundaries,
    /// making the chunks appear as a single continuous stream of bytes.
    ///
    /// ## Arguments
    ///
    /// * `cid` - The CID of the data to read
    ///
    /// ## Returns
    ///
    /// Returns a boxed async reader that can be used to read the data.
    ///
    /// ## Errors
    ///
    /// Returns `StoreError::BlockNotFound` if no block exists with the given CID.
    async fn get_bytes(&self, cid: &Cid) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>>;

    /// Returns the total size in bytes of all blocks associated with a CID.
    ///
    /// For chunked data, this returns the sum of all chunk sizes.
    ///
    /// ## Arguments
    ///
    /// * `cid` - The CID to get the size for
    ///
    /// ## Returns
    ///
    /// Returns the total size in bytes.
    ///
    /// ## Errors
    ///
    /// Returns `StoreError::BlockNotFound` if no block exists with the given CID.
    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64>;

    /// Checks if a block exists in the store.
    ///
    /// ## Arguments
    ///
    /// * `cid` - The CID to check for
    ///
    /// ## Returns
    ///
    /// Returns true if the block exists, false otherwise.
    async fn has(&self, cid: &Cid) -> bool;

    /// Returns the set of IPLD codecs that this store supports.
    ///
    /// The supported codecs determine what types of IPLD data can be stored and retrieved.
    /// Common codecs include Raw, DAG-CBOR, DAG-JSON, and DAG-PB.
    ///
    /// ## Returns
    ///
    /// Returns a set of supported codecs.
    async fn get_supported_codecs(&self) -> HashSet<Codec>;

    /// Returns the maximum allowed size for IPLD nodes and merkle nodes.
    ///
    /// ## Returns
    ///
    /// Returns the size limit in bytes, or None if there is no limit.
    ///
    /// ## Errors
    ///
    /// Returns an error if the store cannot determine its size limit.
    async fn get_max_node_block_size(&self) -> StoreResult<Option<u64>>;

    /// Checks if the store contains any blocks.
    ///
    /// ## Returns
    ///
    /// Returns true if the store has no blocks, false otherwise.
    ///
    /// ## Errors
    ///
    /// Returns an error if the store cannot determine if it's empty.
    async fn is_empty(&self) -> StoreResult<bool> {
        let count = self.get_block_count().await?;
        Ok(count == 0)
    }

    /// Returns the total number of blocks in the store.
    ///
    /// ## Returns
    ///
    /// Returns the number of blocks.
    ///
    /// ## Errors
    ///
    /// Returns an error if the store cannot count its blocks.
    async fn get_block_count(&self) -> StoreResult<u64>;

    /// Indicates whether this store supports garbage collection.
    ///
    /// ## Returns
    ///
    /// Returns true if garbage collection is supported, false otherwise.
    async fn supports_garbage_collection(&self) -> bool {
        false
    }

    /// Attempts to remove a block and its dependencies if they are no longer referenced.
    ///
    /// A block is considered unreferenced when it is no longer needed by any other blocks in the store.
    /// When a block is removed, its dependencies should be checked to see if they are still needed.
    /// If any dependencies are no longer needed, they should also be considered for removal.
    ///
    /// ## Arguments
    ///
    /// * `cid` - The CID of the block to start garbage collection from
    ///
    /// ## Returns
    ///
    /// Returns a set of CIDs that were removed during garbage collection. The set will be empty
    /// if the initial block could not be removed (e.g., if it still has references to it).
    async fn garbage_collect(&self, _cid: &Cid) -> StoreResult<HashSet<Cid>> {
        Ok(HashSet::new())
    }
}

/// A trait for stores that support raw blocks.
///
/// ## Important
///
/// This is a low-level API intended for code implementing an [`IpldStore`].
/// Users should prefer the higher-level methods from [`IpldStore`] instead:
/// - Use [`IpldStore::put_bytes`]/[`IpldStore::get_bytes`] for raw bytes
/// - Use [`IpldStore::put_node`]/[`IpldStore::get_node`] for structured data
#[async_trait]
pub trait RawStore: Clone {
    /// Tries to save `bytes` as a single block to the store. Unlike [`IpldStore::put_bytes`], this
    /// method does not chunk the data and does not create intermediate merkle nodes.
    ///
    /// ## Arguments
    ///
    /// - `bytes`: The bytes to save.
    /// - `is_node`: If true, the block is considered a node block and the size is checked against
    ///   the node block size.
    ///
    /// ## Important
    ///
    /// This is a low-level API intended for code implementing an [`IpldStore`].
    /// Users should prefer [`IpldStore::put_bytes`] or [`IpldStore::put_node`] instead.
    ///
    /// ## Errors
    ///
    /// If the bytes are too large, `StoreError::RawBlockTooLarge` error is returned.
    async fn put_raw_block(&self, bytes: impl Into<Bytes> + Send) -> StoreResult<Cid>;

    /// Retrieves raw bytes of a single block from the store by its `Cid`.
    ///
    /// Unlike [`IpldStore::get_bytes`], this method does not expect chunked data and does not have
    /// to retrieve intermediate merkle nodes.
    ///
    /// ## Important
    ///
    /// This is a low-level API intended for code implementing an [`IpldStore`].
    /// Users should prefer [`IpldStore::get_bytes`] or [`IpldStore::get_node`] instead.
    ///
    /// ## Errors
    ///
    /// If the block is not found, `StoreError::BlockNotFound` error is returned.
    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes>;

    /// Returns the allowed maximum block size for raw bytes.
    ///
    /// ## Returns
    ///
    /// Returns the size limit in bytes, or None if there is no limit.
    async fn get_max_raw_block_size(&self) -> StoreResult<Option<u64>>;
}

/// Helper extension to the `IpldStore` trait.
pub trait IpldStoreExt: IpldStore {
    /// Reads all the bytes associated with the given CID into a single [`Bytes`] type.
    fn read_all(&self, cid: &Cid) -> impl Future<Output = StoreResult<Bytes>> {
        async {
            let mut reader = self.get_bytes(cid).await?;
            let mut bytes = Vec::new();

            reader
                .read_to_end(&mut bytes)
                .await
                .map_err(StoreError::custom)?;

            Ok(Bytes::from(bytes))
        }
    }
}

/// `IpldStoreSeekable` is a trait that extends the `IpldStore` trait to allow for seeking.
#[async_trait]
pub trait IpldStoreSeekable: IpldStore {
    /// Gets a seekable reader for the underlying bytes associated with the given CID.
    async fn get_seekable_bytes(
        &self,
        cid: &Cid,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send + 'static>>>;
}

/// A trait for types that can be changed to a different store.
pub trait StoreSwitchable {
    /// The type of the entity.
    type WithStore<U: IpldStore>;

    /// Change the store used to persist the entity.
    fn change_store<U: IpldStore>(self, new_store: U) -> Self::WithStore<U>;
}

/// A trait for stores that can be configured.
pub trait StoreConfig {
    /// The type of the configuration.
    type Config: Serialize + DeserializeOwned;

    /// Returns the configuration for the store.
    fn get_config(&self) -> Self::Config;
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl TryFrom<u64> for Codec {
    type Error = StoreError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0x55 => Ok(Codec::Raw),
            0x71 => Ok(Codec::DagCbor),
            0x0129 => Ok(Codec::DagJson),
            0x70 => Ok(Codec::DagPb),
            v => Ok(Codec::Unknown(v)),
        }
    }
}

impl From<Codec> for u64 {
    fn from(codec: Codec) -> Self {
        match codec {
            Codec::Raw => 0x55,
            Codec::DagCbor => 0x71,
            Codec::DagJson => 0x0129,
            Codec::DagPb => 0x70,
            Codec::Unknown(value) => value,
        }
    }
}

impl<T> IpldStoreExt for T where T: IpldStore {}

use std::{collections::HashSet, path::PathBuf, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use ipldstore::{
    ipld::cid::Cid, Codec, DualStore, DualStoreConfig, IpldReferences, IpldStore, RawStore,
    StoreResult,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncRead;

use super::FlatFsStore;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An [`IpldStore`] implementation that provides a layered filesystem storage approach with a write layer
/// on top of a read-only base layer.
///
/// ## Architecture
/// The store consists of two layers:
/// 1. Write Layer: A mutable [`FlatFsStore`] where all new writes are directed, with reference counting enabled
/// 2. Base Layer: An immutable [`FlatFsStore`] that serves as a read-only foundation, with reference counting disabled
///
/// ## Read Behavior
/// When reading data (via `get_node`, `get_bytes`, etc.), the store:
/// 1. First checks the write layer for the requested data
/// 2. If not found, falls back to checking the base layer
/// 3. Returns the data from whichever layer it was found in first
///
/// ## Write Behavior
/// - All writes are directed exclusively to the write layer
/// - The base layer remains completely immutable
/// - Reference counting is only enabled in the write layer
/// - The base layer has reference counting explicitly disabled for optimal read-only performance
///
/// ## Example
/// ```ignore
/// let store = LayeredFsStore::new(
///     "path/to/write/layer",     // Where new writes go (reference counting enabled)
///     "path/to/base/layer",      // Read-only foundation (reference counting disabled)
/// );
///
/// // All writes go to the write layer
/// let cid = store.put_node(&my_data).await?;
///
/// // Reads check write layer first, then base layer
/// let data = store.get_node(&cid).await?;
/// ```
///
/// ## Implementation Notes
/// - The base layer has reference counting explicitly disabled for performance
/// - The write layer maintains its own reference counts independently
/// - Both layers use the [`FlatFsStore`] implementation but with different configurations
/// - The base layer's reference counting is disabled.
#[derive(Debug, Clone)]
pub struct LayeredFsStore {
    inner: DualStore<
        // Write store - mutable layer for new writes
        FlatFsStore,
        // Base store - immutable foundation layer
        FlatFsStore,
    >,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl LayeredFsStore {
    /// Creates a new `LayeredFsStore` with separate paths for the write and base layers.
    pub fn new(write_store_path: impl Into<PathBuf>, base_store_path: impl Into<PathBuf>) -> Self {
        Self {
            inner: DualStore::new(
                FlatFsStore::new(write_store_path),
                FlatFsStore::builder()
                    .path(base_store_path)
                    .enable_refcount(false)
                    .build(),
                DualStoreConfig::default(),
            ),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl IpldStore for LayeredFsStore {
    async fn put_node<T>(&self, data: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        self.inner.put_node(data).await
    }

    async fn put_bytes(&self, reader: impl AsyncRead + Send + Sync) -> StoreResult<Cid> {
        self.inner.put_bytes(reader).await
    }

    async fn get_node<T>(&self, cid: &Cid) -> StoreResult<T>
    where
        T: DeserializeOwned + Send,
    {
        self.inner.get_node(cid).await
    }

    async fn get_bytes(&self, cid: &Cid) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        self.inner.get_bytes(cid).await
    }

    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64> {
        self.inner.get_bytes_size(cid).await
    }

    async fn has(&self, cid: &Cid) -> bool {
        self.inner.has(cid).await
    }

    async fn get_supported_codecs(&self) -> HashSet<Codec> {
        self.inner.get_supported_codecs().await
    }

    async fn get_max_node_block_size(&self) -> StoreResult<Option<u64>> {
        self.inner.get_max_node_block_size().await
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        self.inner.get_block_count().await
    }
}

#[async_trait]
impl RawStore for LayeredFsStore {
    async fn put_raw_block(&self, bytes: impl Into<Bytes> + Send) -> StoreResult<Cid> {
        self.inner.put_raw_block(bytes).await
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        self.inner.get_raw_block(cid).await
    }

    async fn get_max_raw_block_size(&self) -> StoreResult<Option<u64>> {
        self.inner.get_max_raw_block_size().await
    }
}

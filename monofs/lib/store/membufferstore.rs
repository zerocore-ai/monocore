use std::{collections::HashSet, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use monoutils_store::{
    ipld::cid::Cid, Codec, DualStore, DualStoreConfig, IpldReferences, IpldStore, MemoryStore,
    RawStore, StoreResult,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncRead;

//--------------------------------------------------------------------------------------------------
// Types: MemoryBufferStore
//--------------------------------------------------------------------------------------------------

/// An [`IpldStore`] with two underlying stores: an ephemeral in-memory
/// store for writes and a user-provided store for back-up reads.
///
/// This store is useful for creating a temporary buffer for writes that is stored in memory.
#[derive(Debug, Clone)]
pub struct MemoryBufferStore<S>
where
    S: IpldStore,
{
    inner: DualStore<MemoryStore, S>,
}

//--------------------------------------------------------------------------------------------------
// Methods: MemoryBufferStore
//--------------------------------------------------------------------------------------------------

impl<S> MemoryBufferStore<S>
where
    S: IpldStore,
{
    /// Creates a new `MemoryBufferStore` with the given backup store.
    pub fn new(backup_store: S) -> Self {
        Self {
            inner: DualStore::new(
                MemoryStore::default(),
                backup_store,
                DualStoreConfig::default(),
            ),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<S> IpldStore for MemoryBufferStore<S>
where
    S: IpldStore + Sync,
{
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
impl<S> RawStore for MemoryBufferStore<S>
where
    S: IpldStore + Sync,
{
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

use std::{collections::HashSet, pin::Pin};

use bytes::Bytes;
use monoutils_store::{
    ipld::cid::Cid, Codec, DualStore, DualStoreConfig, IpldReferences, IpldStore, MemoryStore,
    StoreResult,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncRead;

//--------------------------------------------------------------------------------------------------
// Types: MemoryBufferStore
//--------------------------------------------------------------------------------------------------

/// An [`IpldStore`][zeroutils_store::IpldStore] with two underlying stores: an ephemeral in-memory
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

    async fn put_bytes<'a>(
        &'a self,
        reader: impl AsyncRead + Send + Sync + 'a,
    ) -> StoreResult<Cid> {
        self.inner.put_bytes(reader).await
    }

    async fn put_raw_block(&self, bytes: impl Into<Bytes> + Send) -> StoreResult<Cid> {
        self.inner.put_raw_block(bytes).await
    }

    async fn get_node<T>(&self, cid: &Cid) -> StoreResult<T>
    where
        T: DeserializeOwned + Send,
    {
        self.inner.get_node(cid).await
    }

    async fn get_bytes<'a>(
        &'a self,
        cid: &'a Cid,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send + Sync + 'a>>> {
        self.inner.get_bytes(cid).await
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        self.inner.get_raw_block(cid).await
    }

    #[inline]
    async fn has(&self, cid: &Cid) -> bool {
        self.inner.has(cid).await
    }

    fn get_supported_codecs(&self) -> HashSet<Codec> {
        self.inner.get_supported_codecs()
    }

    #[inline]
    fn get_node_block_max_size(&self) -> Option<u64> {
        self.inner.get_node_block_max_size()
    }

    #[inline]
    fn get_raw_block_max_size(&self) -> Option<u64> {
        self.inner.get_raw_block_max_size()
    }

    async fn is_empty(&self) -> StoreResult<bool> {
        self.inner.is_empty().await
    }

    async fn get_size(&self) -> StoreResult<u64> {
        self.inner.get_size().await
    }
}

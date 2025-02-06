use std::{collections::HashSet, path::PathBuf, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use monoutils_store::{
    ipld::cid::Cid, Codec, DualStore, DualStoreConfig, IpldReferences, IpldStore,
    RawStore, StoreResult,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncRead;

use super::FlatFsStore;

//--------------------------------------------------------------------------------------------------
// Types: LayeredStore
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LayeredStore {
    inner: DualStore<
        // Write store
        FlatFsStore,
        // Base store
        FlatFsStore,
    >,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl LayeredStore {
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
impl IpldStore for LayeredStore {
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
impl RawStore for LayeredStore {
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

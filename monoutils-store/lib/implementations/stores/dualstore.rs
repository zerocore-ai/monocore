use std::{collections::HashSet, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use ipld_core::cid::Cid;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncRead;

use crate::{Codec, IpldReferences, IpldStore, RawStore, StoreError, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A dual store that stores blocks on two different stores.
#[derive(Debug, Clone)]
pub struct DualStore<A, B>
where
    A: IpldStore,
    B: IpldStore,
{
    store_a: A,
    store_b: B,
    config: DualStoreConfig,
}

/// Choices for selecting which store to use for a given operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    /// Use the first store.
    A,
    /// Use the second store.
    B,
}

/// Configuration for a dual store.
#[derive(Debug, Clone)]
pub struct DualStoreConfig {
    /// The default store to use.
    pub default: Choice,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<A, B> DualStore<A, B>
where
    A: IpldStore,
    B: IpldStore,
{
    /// Creates a new dual store from two stores.
    pub fn new(store_a: A, store_b: B, config: DualStoreConfig) -> Self {
        Self {
            store_a,
            store_b,
            config,
        }
    }

    /// Gets the type stored as an IPLD data from a chosen store by its `Cid`.
    pub async fn get_node_from<D>(&self, cid: &Cid, choice: Choice) -> StoreResult<D>
    where
        D: DeserializeOwned + Send,
    {
        match choice {
            Choice::A => self.store_a.get_node(cid).await,
            Choice::B => self.store_b.get_node(cid).await,
        }
    }

    /// Gets the bytes stored in a chosen store as raw bytes by its `Cid`.
    pub async fn get_bytes_from(
        &self,
        cid: &Cid,
        choice: Choice,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        match choice {
            Choice::A => self.store_a.get_bytes(cid).await,
            Choice::B => self.store_b.get_bytes(cid).await,
        }
    }

    /// Gets the size of all the blocks associated with the given `Cid` in bytes.
    pub async fn get_bytes_size_from(&self, cid: &Cid, choice: Choice) -> StoreResult<u64> {
        match choice {
            Choice::A => self.store_a.get_bytes_size(cid).await,
            Choice::B => self.store_b.get_bytes_size(cid).await,
        }
    }

    /// Gets raw bytes from a chosen store as a single block by its `Cid`.
    pub async fn get_raw_block_from(&self, cid: &Cid, choice: Choice) -> StoreResult<Bytes> {
        match choice {
            Choice::A => self.store_a.get_raw_block(cid).await,
            Choice::B => self.store_b.get_raw_block(cid).await,
        }
    }

    /// Saves a serializable type to a chosen store and returns the `Cid` to it.
    pub async fn put_node_into<T>(&self, data: &T, choice: Choice) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        match choice {
            Choice::A => self.store_a.put_node(data).await,
            Choice::B => self.store_b.put_node(data).await,
        }
    }

    /// Saves raw bytes to a chosen store and returns the `Cid` to it.
    pub async fn put_bytes_into(
        &self,
        bytes: impl AsyncRead + Send + Sync,
        choice: Choice,
    ) -> StoreResult<Cid> {
        match choice {
            Choice::A => self.store_a.put_bytes(bytes).await,
            Choice::B => self.store_b.put_bytes(bytes).await,
        }
    }

    /// Saves raw bytes as a single block to a chosen store and returns the `Cid` to it.
    pub async fn put_raw_block_into(
        &self,
        bytes: impl Into<Bytes> + Send,
        choice: Choice,
    ) -> StoreResult<Cid> {
        match choice {
            Choice::A => self.store_a.put_raw_block(bytes).await,
            Choice::B => self.store_b.put_raw_block(bytes).await,
        }
    }

    /// Checks if a block exists in a chosen store by its `Cid`.
    pub async fn has_from(&self, cid: &Cid, choice: Choice) -> bool {
        match choice {
            Choice::A => self.store_a.has(cid).await,
            Choice::B => self.store_b.has(cid).await,
        }
    }
}

impl Choice {
    /// Returns the other choice.
    pub fn other(&self) -> Self {
        match self {
            Choice::A => Choice::B,
            Choice::B => Choice::A,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<A, B> IpldStore for DualStore<A, B>
where
    A: IpldStore + Sync,
    B: IpldStore + Sync,
{
    async fn put_node<T>(&self, data: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        self.put_node_into(data, self.config.default).await
    }

    async fn put_bytes(&self, bytes: impl AsyncRead + Send + Sync) -> StoreResult<Cid> {
        self.put_bytes_into(bytes, self.config.default).await
    }

    async fn get_node<D>(&self, cid: &Cid) -> StoreResult<D>
    where
        D: DeserializeOwned + Send,
    {
        match self.get_node_from(cid, self.config.default).await {
            Ok(data) => Ok(data),
            Err(StoreError::BlockNotFound(_)) => {
                let choice = self.config.default.other();
                self.get_node_from(cid, choice).await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_bytes(&self, cid: &Cid) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        match self.get_bytes_from(cid, self.config.default).await {
            Ok(bytes) => Ok(bytes),
            Err(StoreError::BlockNotFound(_)) => {
                let choice = self.config.default.other();
                self.get_bytes_from(cid, choice).await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64> {
        self.get_bytes_size_from(cid, self.config.default).await
    }

    async fn has(&self, cid: &Cid) -> bool {
        match self.has_from(cid, self.config.default).await {
            true => true,
            false => self.has_from(cid, self.config.default.other()).await,
        }
    }

    async fn get_supported_codecs(&self) -> HashSet<Codec> {
        let mut codecs = self.store_a.get_supported_codecs().await;
        codecs.extend(self.store_b.get_supported_codecs().await);
        codecs
    }

    async fn get_node_block_max_size(&self) -> StoreResult<Option<u64>> {
        let max_size_a = self.store_a.get_node_block_max_size().await?;
        let max_size_b = self.store_b.get_node_block_max_size().await?;
        Ok(max_size_a.max(max_size_b))
    }

    async fn is_empty(&self) -> StoreResult<bool> {
        Ok(self.store_a.is_empty().await? && self.store_b.is_empty().await?)
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        Ok(self.store_a.get_block_count().await? + self.store_b.get_block_count().await?)
    }
}

#[async_trait]
impl<A, B> RawStore for DualStore<A, B>
where
    A: IpldStore + Sync,
    B: IpldStore + Sync,
{
    async fn put_raw_block(&self, bytes: impl Into<Bytes> + Send) -> StoreResult<Cid> {
        self.put_raw_block_into(bytes, self.config.default).await
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        match self.get_raw_block_from(cid, self.config.default).await {
            Ok(bytes) => Ok(bytes),
            Err(StoreError::BlockNotFound(_)) => {
                let choice = self.config.default.other();
                self.get_raw_block_from(cid, choice).await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_raw_block_max_size(&self) -> StoreResult<Option<u64>> {
        let max_size_a = self.store_a.get_raw_block_max_size().await?;
        let max_size_b = self.store_b.get_raw_block_max_size().await?;
        Ok(max_size_a.max(max_size_b))
    }
}

impl Default for DualStoreConfig {
    fn default() -> Self {
        Self { default: Choice::A }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::MemoryStore;

    use super::*;

    #[tokio::test]
    async fn test_dual_store_put_and_get() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();
        let dual_store = DualStore::new(store_a, store_b, Default::default());

        let cid_0 = dual_store.put_node_into(&"hello", Choice::A).await?;
        let cid_1 = dual_store.put_node_into(&250, Choice::B).await?;
        let cid_2 = dual_store.put_node_into(&"world", Choice::A).await?;
        let cid_3 = dual_store.put_node_into(&500, Choice::B).await?;

        assert_eq!(dual_store.get_node::<String>(&cid_0).await?, "hello");
        assert_eq!(dual_store.get_node::<usize>(&cid_1).await?, 250);
        assert_eq!(dual_store.get_node::<String>(&cid_2).await?, "world");
        assert_eq!(dual_store.get_node::<usize>(&cid_3).await?, 500);

        Ok(())
    }
}

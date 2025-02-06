use std::{collections::HashSet, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use monoutils_store::{
    ipld::{cid::Cid, ipld::Ipld},
    Codec, DualStore, DualStoreConfig, IpldReferences, IpldStore, MemoryStore, RawStore,
    StoreError, StoreResult,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_ipld_dagcbor;
use tokio::io::AsyncRead;

use crate::FsResult;

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
    inner: DualStore<
        // Write buffer store
        MemoryStore,
        // Backup store
        S,
    >,
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

    /// Flushes all blocks from the memory buffer to the backup store and clears the buffer.
    ///
    /// This method will:
    /// 1. Copy all blocks from the memory buffer to the backup store, preserving their codec type
    /// 2. For DagCbor blocks, properly handle IPLD references
    /// 3. Clear the memory buffer after successful copying
    ///
    /// ## Returns
    ///
    /// Returns the number of blocks that were flushed to the backup store.
    pub async fn flush(&self) -> FsResult<u64> {
        let memory_store = self.inner.get_store_a();
        let backup_store = self.inner.get_store_b();
        let mut blocks_flushed = 0;

        // Get all CIDs from memory store
        let block_count = memory_store.get_block_count().await?;
        if block_count == 0 {
            return Ok(0);
        }

        // Get a reference to the blocks in memory store
        let blocks = memory_store.get_blocks().read().await;

        // For each block in memory store
        for (cid, (_, block_data)) in blocks.iter() {
            // Skip if block already exists in backup store
            if backup_store.has(cid).await {
                continue;
            }

            // Handle the block based on its codec
            match cid.codec().try_into()? {
                // For raw blocks, just copy them directly
                Codec::Raw => {
                    backup_store.put_raw_block(block_data.clone()).await?;
                    blocks_flushed += 1;
                }
                // For DagCbor blocks, deserialize to preserve references
                Codec::DagCbor => {
                    // Deserialize the block to Ipld to preserve references
                    let ipld: Ipld = serde_ipld_dagcbor::from_slice(block_data)?;

                    // Put the node in the backup store, which will handle reference counting
                    backup_store.put_node(&ipld).await?;
                    blocks_flushed += 1;
                }
                // Return error for unsupported codecs
                codec => {
                    return Err(StoreError::UnexpectedBlockCodec(Codec::DagCbor, codec).into());
                }
            }
        }

        // Drop the read lock before clearing
        drop(blocks);

        // Clear the memory store after successful flush
        memory_store.clear().await?;

        Ok(blocks_flushed)
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use monoutils_store::MemoryStore;

    #[tokio::test]
    async fn test_flush_empty_store() -> anyhow::Result<()> {
        let backup_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(backup_store.clone());

        // Flushing an empty store should return 0
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_flush_with_data() -> anyhow::Result<()> {
        let backup_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(backup_store.clone());

        // Add some data to the buffer
        let data1 = b"test data 1".to_vec();
        let data2 = b"test data 2".to_vec();
        let cid1 = buffer_store.put_raw_block(data1.clone()).await?;
        let cid2 = buffer_store.put_raw_block(data2.clone()).await?;

        // Verify data is in buffer but not in backup
        assert!(buffer_store.has(&cid1).await);
        assert!(buffer_store.has(&cid2).await);
        assert!(!backup_store.has(&cid1).await);
        assert!(!backup_store.has(&cid2).await);

        // Flush the buffer
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 2);

        // Verify data is now in backup store
        assert!(backup_store.has(&cid1).await);
        assert!(backup_store.has(&cid2).await);

        // Verify buffer is cleared
        assert_eq!(buffer_store.inner.get_store_a().get_block_count().await?, 0);

        // Verify we can still read the data through the buffer store
        let retrieved1 = buffer_store.get_raw_block(&cid1).await?;
        let retrieved2 = buffer_store.get_raw_block(&cid2).await?;
        assert_eq!(retrieved1.as_ref(), data1.as_slice());
        assert_eq!(retrieved2.as_ref(), data2.as_slice());

        Ok(())
    }

    #[tokio::test]
    async fn test_flush_with_existing_data() -> anyhow::Result<()> {
        let backup_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(backup_store.clone());

        // Add data to both stores
        let data1 = b"test data 1".to_vec();
        let data2 = b"test data 2".to_vec();

        // Put data1 in both stores
        let cid1 = backup_store.put_raw_block(data1.clone()).await?;
        buffer_store.put_raw_block(data1.clone()).await?;

        // Put data2 only in buffer
        let cid2 = buffer_store.put_raw_block(data2.clone()).await?;

        // Flush should only transfer data2
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 1);

        // Verify both pieces of data are in backup store
        assert!(backup_store.has(&cid1).await);
        assert!(backup_store.has(&cid2).await);

        // Verify buffer is cleared
        assert_eq!(buffer_store.inner.get_store_a().get_block_count().await?, 0);

        Ok(())
    }
}

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

/// A write-through caching [`IpldStore`] that combines an in-memory write buffer with a persistent store.
///
/// This store implements a write-through caching pattern with two layers:
/// 1. A fast, ephemeral in-memory buffer store for writes
/// 2. A persistent underlying store that serves as the source of truth
///
/// ## Write Behavior
/// - All writes go to the memory buffer first
/// - Data in the buffer can be explicitly flushed to the underlying store using [`flush`](Self::flush)
/// - The buffer is cleared after a successful flush
///
/// ## Read Behavior
/// - Reads check the memory buffer first
/// - If not found in the buffer, falls back to reading from the underlying store
///
/// ## Use Cases
/// This store is particularly useful when you need to:
/// - Buffer multiple writes in memory before persisting them
/// - Optimize write performance by batching writes to the underlying store
/// - Maintain data durability by having a persistent underlying store
///
/// ## Example
/// ```ignore
/// let underlying_store = FlatFsStore::new(path);
/// let buffer_store = MemoryBufferStore::new(underlying_store);
///
/// // Write structured data using put_node
/// let node = MyStruct { /* ... */ };
/// let node_cid = buffer_store.put_node(&node).await?;
///
/// // Write raw bytes using put_bytes (preferred over put_raw_block)
/// let bytes = /* ... */;
/// let bytes_cid = buffer_store.put_bytes(bytes).await?;
///
/// // Later, flush buffer to underlying store
/// let blocks_flushed = buffer_store.flush().await?;
/// ```
///
/// ## API Usage
/// For storing data, prefer these high-level APIs:
/// - [`put_node`](IpldStore::put_node) for storing structured data (IPLD nodes)
/// - [`put_bytes`](IpldStore::put_bytes) for storing raw bytes
///
/// Avoid using [`put_raw_block`](RawStore::put_raw_block) directly as it's a low-level API
/// intended for implementing stores, not for general use.
#[derive(Debug, Clone)]
pub struct MemoryBufferStore<S>
where
    S: IpldStore,
{
    inner: DualStore<
        // Buffer store
        MemoryStore,
        // Underlying store
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
    /// Creates a new `MemoryBufferStore` with the given underlying store.
    pub fn new(underlying_store: S) -> Self {
        Self {
            inner: DualStore::new(
                MemoryStore::default(),
                underlying_store,
                DualStoreConfig::default(),
            ),
        }
    }

    /// Flushes all blocks from the memory buffer to the underlying store and clears the buffer.
    ///
    /// This method will:
    /// 1. Copy all blocks from the memory buffer to the underlying store, preserving their codec type
    /// 2. For DagCbor blocks, properly handle IPLD references
    /// 3. Clear the memory buffer after successful copying
    ///
    /// ## Returns
    ///
    /// Returns the number of blocks that were flushed to the underlying store.
    pub async fn flush(&self) -> FsResult<u64> {
        let memory_store = self.inner.get_store_a();
        let underlying_store = self.inner.get_store_b();
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
            // Skip if block already exists in underlying store
            if underlying_store.has(cid).await {
                continue;
            }

            // Handle the block based on its codec
            match cid.codec().try_into()? {
                // For raw blocks, just copy them directly
                Codec::Raw => {
                    underlying_store.put_raw_block(block_data.clone()).await?;
                    blocks_flushed += 1;
                }
                // For DagCbor blocks, deserialize to preserve references
                Codec::DagCbor => {
                    // Deserialize the block to Ipld to preserve references
                    let ipld: Ipld = serde_ipld_dagcbor::from_slice(block_data)?;

                    // Put the node in the underlying store, which will handle reference counting
                    underlying_store.put_node(&ipld).await?;
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

    use super::helper::TestNode;

    #[tokio::test]
    async fn test_memory_buffer_store_flush_empty_store() -> anyhow::Result<()> {
        let underlying_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(underlying_store.clone());

        // Flushing an empty store should return 0
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_buffer_store_flush_with_data() -> anyhow::Result<()> {
        let underlying_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(underlying_store.clone());

        // Add some data to the buffer
        let data1 = b"test data 1".to_vec();
        let data2 = b"test data 2".to_vec();
        let cid1 = buffer_store.put_raw_block(data1.clone()).await?;
        let cid2 = buffer_store.put_raw_block(data2.clone()).await?;

        // Verify data is in buffer but not in underlying store
        assert!(buffer_store.has(&cid1).await);
        assert!(buffer_store.has(&cid2).await);
        assert!(!underlying_store.has(&cid1).await);
        assert!(!underlying_store.has(&cid2).await);

        // Flush the buffer
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 2);

        // Verify data is now in underlying store
        assert!(underlying_store.has(&cid1).await);
        assert!(underlying_store.has(&cid2).await);

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
    async fn test_memory_buffer_store_flush_with_existing_data() -> anyhow::Result<()> {
        let underlying_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(underlying_store.clone());

        // Add data to both stores
        let data1 = b"test data 1".to_vec();
        let data2 = b"test data 2".to_vec();

        // Put data1 in both stores
        let cid1 = underlying_store.put_raw_block(data1.clone()).await?;
        buffer_store.put_raw_block(data1.clone()).await?;

        // Put data2 only in buffer
        let cid2 = buffer_store.put_raw_block(data2.clone()).await?;

        // Flush should only transfer data2
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 1);

        // Verify both pieces of data are in underlying store
        assert!(underlying_store.has(&cid1).await);
        assert!(underlying_store.has(&cid2).await);

        // Verify buffer is cleared
        assert_eq!(buffer_store.inner.get_store_a().get_block_count().await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_buffer_store_flush_with_nodes() -> anyhow::Result<()> {
        let underlying_store = MemoryStore::default();
        let buffer_store = MemoryBufferStore::new(underlying_store.clone());

        // Create some raw data first
        let data1 = b"test data 1".to_vec();
        let data2 = b"test data 2".to_vec();
        let cid1 = buffer_store.put_raw_block(data1.clone()).await?;
        let cid2 = buffer_store.put_raw_block(data2.clone()).await?;

        // Create nodes that reference the raw data
        let leaf_node = TestNode {
            name: "leaf".to_string(),
            value: 1,
            refs: vec![cid1],
        };
        let leaf_cid = buffer_store.put_node(&leaf_node).await?;

        let root_node = TestNode {
            name: "root".to_string(),
            value: 2,
            refs: vec![leaf_cid, cid2],
        };
        let root_cid = buffer_store.put_node(&root_node).await?;

        // Verify data is in buffer but not in underlying store
        assert!(buffer_store.has(&cid1).await);
        assert!(buffer_store.has(&cid2).await);
        assert!(buffer_store.has(&leaf_cid).await);
        assert!(buffer_store.has(&root_cid).await);
        assert!(!underlying_store.has(&cid1).await);
        assert!(!underlying_store.has(&cid2).await);
        assert!(!underlying_store.has(&leaf_cid).await);
        assert!(!underlying_store.has(&root_cid).await);

        // Flush the buffer
        let flushed = buffer_store.flush().await?;
        assert_eq!(flushed, 4); // Should flush all 4 blocks

        // Verify all data is now in underlying store
        assert!(underlying_store.has(&cid1).await);
        assert!(underlying_store.has(&cid2).await);
        assert!(underlying_store.has(&leaf_cid).await);
        assert!(underlying_store.has(&root_cid).await);

        // Verify buffer is cleared
        assert_eq!(buffer_store.inner.get_store_a().get_block_count().await?, 0);

        // Verify we can still read the data through the buffer store
        let retrieved_root: TestNode = buffer_store.get_node(&root_cid).await?;
        assert_eq!(retrieved_root.name, "root");
        assert_eq!(retrieved_root.value, 2);
        assert_eq!(retrieved_root.refs, vec![leaf_cid, cid2]);

        let retrieved_leaf: TestNode = buffer_store.get_node(&leaf_cid).await?;
        assert_eq!(retrieved_leaf.name, "leaf");
        assert_eq!(retrieved_leaf.value, 1);
        assert_eq!(retrieved_leaf.refs, vec![cid1]);

        // Verify raw blocks are also accessible
        let retrieved1 = buffer_store.get_raw_block(&cid1).await?;
        let retrieved2 = buffer_store.get_raw_block(&cid2).await?;
        assert_eq!(retrieved1.as_ref(), data1.as_slice());
        assert_eq!(retrieved2.as_ref(), data2.as_slice());

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub(super) struct TestNode {
        pub(super) name: String,
        pub(super) value: i32,
        pub(super) refs: Vec<Cid>,
    }

    impl IpldReferences for TestNode {
        fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
            Box::new(self.refs.iter())
        }
    }
}

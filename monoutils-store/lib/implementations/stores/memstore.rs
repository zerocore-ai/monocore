use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use ipld_core::{cid::Cid, codec::Links};
use monoutils::SeekableReader;
use serde::{de::DeserializeOwned, Serialize};
use serde_ipld_dagcbor::codec::DagCborCodec;
use tokio::{io::AsyncRead, sync::RwLock};

use crate::{
    utils, Chunker, Codec, FastCDCChunker, FixedSizeChunker, FlatLayout, IpldReferences, IpldStore,
    IpldStoreSeekable, Layout, LayoutSeekable, RawStore, StoreError, StoreResult,
    DEFAULT_MAX_NODE_BLOCK_SIZE,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An in-memory storage for IPLD nodes and bytes.
///
/// This store provides a thread-safe in-memory implementation for storing [IPLD (InterPlanetary
/// Linked Data)][ipld] nodes and raw bytes. It supports:
///
/// - Storage of both IPLD nodes (DagCbor encoded) and raw bytes
/// - Content-addressed storage using [CIDs][cid]
/// - Automatic chunking of large data using configurable chunking strategies
/// - Flexible data layout patterns through the Layout trait
/// - Reference counting for automatic garbage collection
///
/// The store maintains a reference count for each block, tracking both direct storage and references
/// from other IPLD nodes. This reference counting is particularly effective for IPLD data since it
/// forms a directed acyclic graph (DAG), meaning there can never be cyclical references. This
/// property ensures reference counting will correctly identify unreachable blocks when they are
/// no longer referenced.
///
/// When a block's reference count reaches zero, it becomes eligible for garbage collection along
/// with any of its referenced blocks (recursively) that also reach zero references.
///
/// [cid]: https://docs.ipfs.tech/concepts/content-addressing/
/// [ipld]: https://ipld.io/
#[derive(Debug, Clone)]
// TODO: Use `BalancedDagLayout` as default
pub struct MemoryStore<C = FixedSizeChunker, L = FlatLayout>
where
    C: Chunker,
    L: Layout,
{
    /// Represents the blocks stored in the store.
    ///
    /// When data is added to the store, it may not necessarily fit into the acceptable block size
    /// limit, so it is chunked into smaller blocks.
    ///
    /// The `usize` is used for counting the references to blocks within the store.
    blocks: Arc<RwLock<HashMap<Cid, (usize, Bytes)>>>,

    /// The chunking algorithm used to split data into chunks.
    chunker: C,

    /// The layout strategy used to store chunked data.
    layout: L,
}

/// The default [`MemoryStore`] using [`FastCDCChunker`] and [`FlatLayout`].
pub type MemoryStoreDefault = MemoryStore<FastCDCChunker, FlatLayout>;

/// A [`MemoryStore`] with a [`FixedSizeChunker`] and [`FlatLayout`].
pub type MemoryStoreFixed = MemoryStore<FixedSizeChunker, FlatLayout>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<C, L> MemoryStore<C, L>
where
    C: Chunker,
    L: Layout,
{
    /// Creates a new `MemoryStore` with default chunker and layout.
    pub fn new() -> Self
    where
        C: Default,
        L: Default,
    {
        Self {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            chunker: C::default(),
            layout: L::default(),
        }
    }

    /// Creates a new `MemoryStore` with the given `chunker` and `layout`.
    pub fn with_chunker_and_layout(chunker: C, layout: L) -> Self {
        MemoryStore {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            chunker,
            layout,
        }
    }

    /// Prints all the blocks in the store.
    pub async fn print_blocks(&self)
    where
        C: Clone + Send,
        L: Clone + Send,
    {
        let store = self.clone();
        let blocks = store.blocks.read().await;
        for (cid, (refcount, bytes)) in blocks.iter() {
            println!("[{:03}] {}: {}", refcount, cid, hex::encode(bytes));
        }
    }

    /// Increments the reference count of the blocks with the given `Cid`s.
    async fn increment_reference_counts(&self, cids: impl Iterator<Item = &Cid>) {
        for cid in cids {
            if let Some((count, _)) = self.blocks.write().await.get_mut(cid) {
                *count += 1;
            }
        }
    }

    /// Stores raw bytes in the store without any size checks.
    /// Returns a tuple of (Cid, bool) where the bool indicates if the data already existed in the store.
    async fn store_raw(&self, bytes: Bytes, codec: Codec) -> (Cid, bool) {
        let cid = utils::generate_cid(codec, &bytes);
        let mut blocks = self.blocks.write().await;
        let existed = blocks.contains_key(&cid);
        if !existed {
            blocks.insert(cid, (0, bytes));
        }
        (cid, existed)
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<C, L> IpldStore for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync + 'static,
    L: Layout + Clone + Send + Sync + 'static,
{
    async fn put_node<T>(&self, node: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        // Serialize the data to bytes.
        let bytes = Bytes::from(serde_ipld_dagcbor::to_vec(&node).map_err(StoreError::custom)?);

        // Check if the data exceeds the node maximum block size.
        if let Some(max_size) = self.get_max_node_block_size().await? {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::NodeBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        let (cid, existed) = self.store_raw(bytes, Codec::DagCbor).await;

        // Only increment reference counts if this is a new entry
        if !existed {
            self.increment_reference_counts(node.get_references()).await;
        }

        Ok(cid)
    }

    async fn put_bytes(&self, reader: impl AsyncRead + Send + Sync) -> StoreResult<Cid> {
        let chunk_stream = self.chunker.chunk(reader).await?;
        let mut cid_stream = self.layout.organize(chunk_stream, self.clone()).await?;

        // Take the last `Cid` from the stream.
        let mut cid = cid_stream.next().await.unwrap()?;
        while let Some(result) = cid_stream.next().await {
            cid = result?;
        }

        Ok(cid)
    }

    async fn get_node<D>(&self, cid: &Cid) -> StoreResult<D>
    where
        D: DeserializeOwned + Send,
    {
        let blocks = self.blocks.read().await;
        match blocks.get(cid) {
            Some((_, bytes)) => match cid.codec().try_into()? {
                Codec::DagCbor => {
                    let data =
                        serde_ipld_dagcbor::from_slice::<D>(bytes).map_err(StoreError::custom)?;
                    Ok(data)
                }
                codec => Err(StoreError::UnexpectedBlockCodec(Codec::DagCbor, codec)),
            },
            None => Err(StoreError::BlockNotFound(*cid)),
        }
    }

    async fn get_bytes(&self, cid: &Cid) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        self.layout.retrieve(cid, self.clone()).await
    }

    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64> {
        self.layout.get_size(cid, self.clone()).await
    }

    async fn has(&self, cid: &Cid) -> bool {
        let blocks = self.blocks.read().await;
        blocks.contains_key(cid)
    }

    async fn get_supported_codecs(&self) -> HashSet<Codec> {
        let mut codecs = HashSet::new();
        codecs.insert(Codec::DagCbor);
        codecs.insert(Codec::Raw);
        codecs
    }

    async fn get_max_node_block_size(&self) -> StoreResult<Option<u64>> {
        Ok(Some(DEFAULT_MAX_NODE_BLOCK_SIZE))
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        Ok(self.blocks.read().await.len() as u64)
    }

    async fn supports_garbage_collection(&self) -> bool {
        true
    }

    async fn garbage_collect(&self, cid: &Cid) -> StoreResult<HashSet<Cid>> {
        let mut removed_cids = HashSet::new();

        // Check if the CID exists and has refcount of exactly 0
        let refs = {
            let mut blocks = self.blocks.write().await;
            match blocks.get(cid) {
                Some((count, bytes)) if *count == 0 => {
                    // Try to deserialize the block to get its references
                    let codec: Codec = cid.codec().try_into()?;
                    match codec {
                        Codec::DagCbor => {
                            // Extract CID references using the Links trait
                            let refs = DagCborCodec::links(&bytes)
                                .map_err(StoreError::custom)?
                                .collect::<Vec<_>>();

                            // Remove the block since refcount is 0
                            blocks.remove(cid);
                            removed_cids.insert(*cid);

                            Some(refs)
                        }
                        Codec::Raw => {
                            // For raw blocks, just remove them if refcount is 0
                            blocks.remove(cid);
                            removed_cids.insert(*cid);
                            None
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        };

        // Process dependencies if we had any
        if let Some(refs) = refs {
            for ref_cid in refs {
                // Decrement refcount and check if we should collect
                let should_collect = {
                    let mut blocks = self.blocks.write().await;
                    if let Some((count, _)) = blocks.get_mut(&ref_cid) {
                        *count = count.saturating_sub(1);
                        *count == 0
                    } else {
                        false
                    }
                };

                // If refcount reached 0, recursively collect it
                if should_collect {
                    let sub_removed = self.garbage_collect(&ref_cid).await?;
                    removed_cids.extend(sub_removed);
                }
            }
        }

        Ok(removed_cids)
    }
}

#[async_trait]
impl<C, L> RawStore for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync,
    L: Layout + Clone + Send + Sync,
{
    async fn put_raw_block(&self, bytes: impl Into<Bytes> + Send) -> StoreResult<Cid> {
        let bytes = bytes.into();
        if let Some(max_size) = self.get_max_raw_block_size().await? {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::RawBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        Ok(self.store_raw(bytes, Codec::Raw).await.0)
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        let blocks = self.blocks.read().await;
        match blocks.get(cid) {
            Some((_, bytes)) => match cid.codec().try_into()? {
                Codec::Raw => Ok(bytes.clone()),
                codec => Err(StoreError::UnexpectedBlockCodec(Codec::Raw, codec)),
            },
            None => Err(StoreError::BlockNotFound(*cid)),
        }
    }

    async fn get_max_raw_block_size(&self) -> StoreResult<Option<u64>> {
        Ok(self
            .chunker
            .chunk_max_size()
            .await?
            .max(Some(DEFAULT_MAX_NODE_BLOCK_SIZE)))
    }
}

#[async_trait]
impl<C, L> IpldStoreSeekable for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync + 'static,
    L: LayoutSeekable + Clone + Send + Sync + 'static,
{
    async fn get_seekable_bytes(
        &self,
        cid: &Cid,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send + 'static>>> {
        self.layout.retrieve_seekable(cid, self.clone()).await
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        MemoryStore {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            chunker: FixedSizeChunker::default(),
            layout: FlatLayout::default(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::DEFAULT_MAX_CHUNK_SIZE;

    use super::helper::TestNode;
    use super::*;
    use multihash_codetable::{Code, MultihashDigest};
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_memory_store_raw_block() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Store a raw block
        let data = b"Hello, World!".to_vec();
        let cid = store.put_raw_block(data.clone()).await?;

        // Verify the block exists
        assert!(store.has(&cid).await);

        // Read it back
        let retrieved = store.get_raw_block(&cid).await?;
        assert_eq!(retrieved.as_ref(), data.as_slice());

        // Verify block count
        assert_eq!(store.get_block_count().await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_store_bytes() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Generate data larger than the default chunk size to trigger chunking
        let data: Vec<u8> = (0..(DEFAULT_MAX_CHUNK_SIZE * 3) as usize)
            .map(|i| (i % 255) as u8)
            .collect();

        let cid = store.put_bytes(data.as_slice()).await?;

        // Verify the size matches
        let size = store.get_bytes_size(&cid).await?;
        assert_eq!(size, data.len() as u64);

        // Read it back using get_bytes
        let mut reader = store.get_bytes(&cid).await?;
        let mut retrieved = Vec::new();
        reader.read_to_end(&mut retrieved).await?;

        assert_eq!(retrieved, data);

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_store_node() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Create and store a node
        let node = TestNode {
            name: "test".to_string(),
            value: 42,
            refs: Vec::new(),
        };
        let cid = store.put_node(&node).await?;

        // Read it back
        let retrieved: TestNode = store.get_node(&cid).await?;
        assert_eq!(retrieved.name, node.name);
        assert_eq!(retrieved.value, node.value);

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_store_error_handling() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Try to get non-existent block
        let non_existent_cid =
            Cid::new_v1(Codec::Raw.into(), Code::Blake3_256.digest(b"non-existent"));
        assert!(!store.has(&non_existent_cid).await);
        assert!(store.get_raw_block(&non_existent_cid).await.is_err());
        assert!(store.get_bytes(&non_existent_cid).await.is_err());

        // Store a raw block and try to read it as a node
        let data = b"raw data".to_vec();
        let cid = store.put_raw_block(data).await?;
        assert!(store.get_node::<TestNode>(&cid).await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_store_operations() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Initially empty
        assert!(store.is_empty().await?);
        assert_eq!(store.get_block_count().await?, 0);

        // Store some blocks
        let data1 = b"block 1".to_vec();
        let data2 = b"block 2".to_vec();
        store.put_raw_block(data1).await?;
        store.put_raw_block(data2).await?;

        // Not empty anymore
        assert!(!store.is_empty().await?);
        assert_eq!(store.get_block_count().await?, 2);

        // Verify supported codecs
        let codecs = store.get_supported_codecs().await;
        assert!(codecs.contains(&Codec::Raw));
        assert!(codecs.contains(&Codec::DagCbor));

        // Verify size limits from chunker
        assert_eq!(
            store.get_max_node_block_size().await?,
            Some(DEFAULT_MAX_NODE_BLOCK_SIZE)
        );
        assert_eq!(
            store.get_max_raw_block_size().await?,
            Some(DEFAULT_MAX_NODE_BLOCK_SIZE)
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_store_garbage_collect() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Create a node that references another block
        let data = b"Hello, World!".to_vec();
        let data_cid = store.put_raw_block(data.clone()).await?;

        let node = TestNode {
            name: "test".to_string(),
            value: 42,
            refs: vec![data_cid],
        };
        let node_cid = store.put_node(&node).await?;

        // Verify both blocks exist
        assert!(store.has(&data_cid).await);
        assert!(store.has(&node_cid).await);

        println!("store before gc");
        store.print_blocks().await;

        // Read back the node to verify refs are properly serialized
        let retrieved_node: TestNode = store.get_node(&node_cid).await?;
        assert_eq!(retrieved_node.refs, vec![data_cid]);

        // Try to garbage collect data_cid - should fail because node references it
        let removed = store.garbage_collect(&data_cid).await?;
        assert!(removed.is_empty());
        assert!(store.has(&data_cid).await);

        println!("\nstore after failed gc");
        store.print_blocks().await;

        // Garbage collect node_cid - should succeed and trigger data_cid collection
        let removed = store.garbage_collect(&node_cid).await?;

        println!("\nstore after successful gc");
        store.print_blocks().await;

        assert!(removed.contains(&node_cid));
        assert!(removed.contains(&data_cid));
        assert!(!store.has(&node_cid).await);
        assert!(!store.has(&data_cid).await);
        assert!(store.is_empty().await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_memory_store_complex_garbage_collect() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Create some raw data blocks
        let data1 = b"Data block 1".to_vec();
        let data2 = b"Data block 2".to_vec();
        let data3 = b"Data block 3".to_vec();
        let data1_cid = store.put_raw_block(data1.clone()).await?;
        let data2_cid = store.put_raw_block(data2.clone()).await?;
        let data3_cid = store.put_raw_block(data3.clone()).await?;

        // ======================== Tree Structure ========================
        //
        //             root[value: 5, rc: 0]
        //                 /            \
        //                /              \
        //               /                \
        //  middle1[value: 3, rc: 1]      middle2[value: 4, rc: 1]
        //        /           \                /              \
        //       /             \              /                \
        //      /               \            /                  \
        //  leaf1[value: 1, rc: 1]   leaf2[value: 2, rc: 2]      \
        //     |                           |                      \
        //     |                           |                       \
        //  data1[rc: 1]             data2[rc: 1]            data3[rc: 1]
        //
        // ===============================================================

        // Create leaf nodes
        let leaf1 = TestNode {
            name: "leaf1".to_string(),
            value: 1,
            refs: vec![data1_cid],
        };
        let leaf2 = TestNode {
            name: "leaf2".to_string(),
            value: 2,
            refs: vec![data2_cid],
        };
        let leaf1_cid = store.put_node(&leaf1).await?;
        let leaf2_cid = store.put_node(&leaf2).await?;

        // Create middle nodes
        let middle1 = TestNode {
            name: "middle1".to_string(),
            value: 3,
            refs: vec![leaf1_cid, leaf2_cid],
        };
        let middle2 = TestNode {
            name: "middle2".to_string(),
            value: 4,
            refs: vec![leaf2_cid, data3_cid], // Note: leaf2 is referenced twice
        };
        let middle1_cid = store.put_node(&middle1).await?;
        let middle2_cid = store.put_node(&middle2).await?;

        // Create root node
        let root = TestNode {
            name: "root".to_string(),
            value: 5,
            refs: vec![middle1_cid, middle2_cid],
        };
        let root_cid = store.put_node(&root).await?;

        println!("\nInitial store state:");
        store.print_blocks().await;

        // Try to collect leaf1 - should fail as it's referenced by middle1
        let removed = store.garbage_collect(&leaf1_cid).await?;
        assert!(
            removed.is_empty(),
            "No nodes should be removed when collecting referenced leaf1"
        );
        assert!(store.has(&leaf1_cid).await);
        assert!(store.has(&data1_cid).await);

        println!("\nAfter attempting to collect leaf1:");
        store.print_blocks().await;

        // Try to collect middle1 - should fail as it's referenced by root
        let removed = store.garbage_collect(&middle1_cid).await?;
        assert!(
            removed.is_empty(),
            "No nodes should be removed when collecting referenced middle1"
        );
        assert!(store.has(&middle1_cid).await);
        assert!(store.has(&leaf1_cid).await);
        assert!(store.has(&leaf2_cid).await);

        println!("\nAfter attempting to collect middle1:");
        store.print_blocks().await;

        // Try to collect data2 - should fail as it's referenced by leaf2
        let removed = store.garbage_collect(&data2_cid).await?;
        assert!(
            removed.is_empty(),
            "No nodes should be removed when collecting referenced data2"
        );
        assert!(store.has(&data2_cid).await);

        println!("\nAfter attempting to collect data2:");
        store.print_blocks().await;

        // Finally collect root node - this should collect everything since root has refcount 0
        let removed = store.garbage_collect(&root_cid).await?;
        // Verify all nodes are in the removed set
        assert!(
            removed.contains(&root_cid),
            "Root node should be in removed set"
        );
        assert!(
            removed.contains(&middle1_cid),
            "Middle1 node should be in removed set"
        );
        assert!(
            removed.contains(&middle2_cid),
            "Middle2 node should be in removed set"
        );
        assert!(
            removed.contains(&leaf1_cid),
            "Leaf1 node should be in removed set"
        );
        assert!(
            removed.contains(&leaf2_cid),
            "Leaf2 node should be in removed set"
        );
        assert!(
            removed.contains(&data1_cid),
            "Data1 block should be in removed set"
        );
        assert!(
            removed.contains(&data2_cid),
            "Data2 block should be in removed set"
        );
        assert!(
            removed.contains(&data3_cid),
            "Data3 block should be in removed set"
        );
        // Verify the size of removed set matches expected number of nodes
        assert_eq!(removed.len(), 8, "Should have removed exactly 8 nodes");
        // Verify nodes are no longer in store
        assert!(!store.has(&root_cid).await);
        assert!(!store.has(&middle1_cid).await);
        assert!(!store.has(&middle2_cid).await);
        assert!(!store.has(&leaf1_cid).await);
        assert!(!store.has(&leaf2_cid).await);
        assert!(!store.has(&data1_cid).await);
        assert!(!store.has(&data2_cid).await);
        assert!(!store.has(&data3_cid).await);
        assert!(store.is_empty().await?);

        println!("\nAfter collecting root:");
        store.print_blocks().await;

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use serde::Deserialize;

    use super::*;

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

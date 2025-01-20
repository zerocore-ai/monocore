use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

use bytes::Bytes;
use futures::StreamExt;
use libipld::Cid;
use monoutils::SeekableReader;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{io::AsyncRead, sync::RwLock};

use crate::{
    utils, Chunker, Codec, FixedSizeChunker, FlatLayout, IpldReferences, IpldStore,
    IpldStoreSeekable, Layout, LayoutSeekable, RawStore, StoreError, StoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An in-memory storage for IPLD node and raw blocks with reference counting.
///
/// This store maintains a reference count for each stored block. Reference counting is used to
/// determine when a block can be safely removed from the store.
#[derive(Debug, Clone)]
// TODO: Use `BalancedDagLayout` as default
// TODO: Use `FastCDCChunker` as default chunker
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

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<C, L> MemoryStore<C, L>
where
    C: Chunker,
    L: Layout,
{
    /// Creates a new `MemoryStore` with the given `chunker` and `layout`.
    pub fn with_chunker_and_layout(chunker: C, layout: L) -> Self {
        MemoryStore {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            chunker,
            layout,
        }
    }

    /// Prints all the blocks in the store.
    pub fn debug(&self)
    where
        C: Clone + Send,
        L: Clone + Send,
    {
        let store = self.clone();
        tokio::spawn(async move {
            let blocks = store.blocks.read().await;
            for (cid, (size, bytes)) in blocks.iter() {
                println!("\ncid: {} ({:?})\nkey: {}", cid, size, hex::encode(bytes));
            }
        });
    }

    /// Increments the reference count of the blocks with the given `Cid`s.
    async fn inc_refs(&self, cids: impl Iterator<Item = &Cid>) {
        for cid in cids {
            if let Some((size, _)) = self.blocks.write().await.get_mut(cid) {
                *size += 1;
            }
        }
    }

    /// Stores raw bytes in the store without any size checks.
    async fn store_raw(&self, bytes: Bytes, codec: Codec) -> Cid {
        let cid = utils::make_cid(codec, &bytes);
        self.blocks.write().await.insert(cid, (1, bytes));
        cid
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<C, L> IpldStore for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync,
    L: Layout + Clone + Send + Sync,
{
    async fn put_node<T>(&self, data: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        // Serialize the data to bytes.
        let bytes = Bytes::from(serde_ipld_dagcbor::to_vec(&data).map_err(StoreError::custom)?);

        // Check if the data exceeds the node maximum block size.
        if let Some(max_size) = self.get_node_block_max_size() {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::NodeBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        // Increment the reference count of the block.
        self.inc_refs(data.get_references()).await;

        Ok(self.store_raw(bytes, Codec::DagCbor).await)
    }

    async fn put_bytes<'a>(
        &'a self,
        reader: impl AsyncRead + Send + Sync + 'a,
    ) -> StoreResult<Cid> {
        let chunk_stream = self.chunker.chunk(reader).await?;
        let mut cid_stream = self.layout.organize(chunk_stream, self.clone()).await?;

        // Take the last `Cid` from the stream.
        let mut cid = cid_stream.next().await.unwrap()?;
        while let Some(result) = cid_stream.next().await {
            cid = result?;
        }

        Ok(cid)
    }

    async fn get_node<T>(&self, cid: &Cid) -> StoreResult<T>
    where
        T: DeserializeOwned,
    {
        let blocks = self.blocks.read().await;
        match blocks.get(cid) {
            Some((_, bytes)) => match cid.codec().try_into()? {
                Codec::DagCbor => {
                    let data =
                        serde_ipld_dagcbor::from_slice::<T>(bytes).map_err(StoreError::custom)?;
                    Ok(data)
                }
                codec => Err(StoreError::UnexpectedBlockCodec(Codec::DagCbor, codec)),
            },
            None => Err(StoreError::BlockNotFound(*cid)),
        }
    }

    async fn get_bytes<'a>(
        &'a self,
        cid: &'a Cid,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send + Sync + 'a>>> {
        self.layout.retrieve(cid, self.clone()).await
    }

    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64> {
        self.layout.get_size(cid, self.clone()).await
    }

    #[inline]
    async fn has(&self, cid: &Cid) -> bool {
        let blocks = self.blocks.read().await;
        blocks.contains_key(cid)
    }

    fn get_supported_codecs(&self) -> HashSet<Codec> {
        let mut codecs = HashSet::new();
        codecs.insert(Codec::DagCbor);
        codecs.insert(Codec::Raw);
        codecs
    }

    #[inline]
    fn get_node_block_max_size(&self) -> Option<u64> {
        self.chunker.chunk_max_size()
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        Ok(self.blocks.read().await.len() as u64)
    }
}

impl<C, L> RawStore for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync,
    L: Layout + Clone + Send + Sync,
{
    async fn put_raw_block(&self, bytes: impl Into<Bytes>) -> StoreResult<Cid> {
        let bytes = bytes.into();
        if let Some(max_size) = self.get_raw_block_max_size() {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::RawBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        Ok(self.store_raw(bytes, Codec::Raw).await)
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

    #[inline]
    fn get_raw_block_max_size(&self) -> Option<u64> {
        self.chunker.chunk_max_size()
    }
}

impl<C, L> IpldStoreSeekable for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync,
    L: LayoutSeekable + Clone + Send + Sync,
{
    async fn get_seekable_bytes<'a>(
        &'a self,
        cid: &'a Cid,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send + Sync + 'a>>> {
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

    use super::fixtures::TestNode;
    use super::*;
    use libipld::multihash::{Code, MultihashDigest};
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

        let cid = store.put_bytes(&data[..]).await?;

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
        let codecs = store.get_supported_codecs();
        assert!(codecs.contains(&Codec::Raw));
        assert!(codecs.contains(&Codec::DagCbor));

        // Verify size limits from chunker
        assert_eq!(
            store.get_node_block_max_size(),
            Some(DEFAULT_MAX_CHUNK_SIZE)
        );
        assert_eq!(store.get_raw_block_max_size(), Some(DEFAULT_MAX_CHUNK_SIZE));

        Ok(())
    }
}

#[cfg(test)]
mod fixtures {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub(super) struct TestNode {
        pub(super) name: String,
        pub(super) value: i32,
        #[serde(skip)]
        pub(super) refs: Vec<Cid>,
    }

    impl IpldReferences for TestNode {
        fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
            Box::new(self.refs.iter())
        }
    }
}

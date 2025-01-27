use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use ipld_core::cid::Cid;
use monoutils::SeekableReader;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{io::AsyncRead, sync::RwLock};

use crate::{
    utils, Chunker, Codec, FastCDCChunker, FixedSizeChunker, FlatLayout, IpldReferences, IpldStore,
    IpldStoreSeekable, Layout, LayoutSeekable, RawStore, StoreError, StoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An in-memory storage for IPLD nodes and bytes.
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

#[async_trait]
impl<C, L> IpldStore for MemoryStore<C, L>
where
    C: Chunker + Clone + Send + Sync + 'static,
    L: Layout + Clone + Send + Sync + 'static,
{
    async fn put_node<T>(&self, data: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        // Serialize the data to bytes.
        let bytes = Bytes::from(serde_ipld_dagcbor::to_vec(&data).map_err(StoreError::custom)?);

        // Check if the data exceeds the node maximum block size.
        if let Some(max_size) = self.get_node_block_max_size().await? {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::NodeBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        // Increment the reference count of the block.
        self.inc_refs(data.get_references()).await;

        Ok(self.store_raw(bytes, Codec::DagCbor).await)
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

    async fn get_node_block_max_size(&self) -> StoreResult<Option<u64>> {
        self.chunker.chunk_max_size().await
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        Ok(self.blocks.read().await.len() as u64)
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
        if let Some(max_size) = self.get_raw_block_max_size().await? {
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

    async fn get_raw_block_max_size(&self) -> StoreResult<Option<u64>> {
        self.chunker.chunk_max_size().await
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
    use std::io::Cursor;

    use crate::DEFAULT_MAX_CHUNK_SIZE;

    use super::fixtures::TestNode;
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

        let cid = store.put_bytes(Cursor::new(data.clone())).await?; // TODO: Hate this clone

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
            store.get_node_block_max_size().await?,
            Some(DEFAULT_MAX_CHUNK_SIZE)
        );
        assert_eq!(
            store.get_raw_block_max_size().await?,
            Some(DEFAULT_MAX_CHUNK_SIZE)
        );

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

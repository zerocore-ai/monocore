use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

use bytes::Bytes;
use futures::StreamExt;
use libipld::Cid;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{io::AsyncRead, sync::RwLock};

use crate::{
    utils, Chunker, Codec, FixedSizeChunker, FlatLayout, IpldReferences, IpldStore,
    IpldStoreSeekable, Layout, LayoutSeekable, SeekableReader, StoreError, StoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An in-memory storage for IPLD node and raw blocks with reference counting.
///
/// This store maintains a reference count for each stored block. Reference counting is used to
/// determine when a block can be safely removed from the store.
#[derive(Debug, Clone)]
// TODO: Use BalancedDagLayout as default
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
    pub fn new(chunker: C, layout: L) -> Self {
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

    async fn put_raw_block(&self, bytes: impl Into<Bytes>) -> StoreResult<Cid> {
        let bytes = bytes.into();
        if let Some(max_size) = self.get_raw_block_max_size() {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::RawBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        Ok(self.store_raw(bytes, Codec::Raw).await)
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

    #[inline]
    fn get_raw_block_max_size(&self) -> Option<u64> {
        self.chunker.chunk_max_size()
    }

    async fn is_empty(&self) -> StoreResult<bool> {
        Ok(self.blocks.read().await.is_empty())
    }

    async fn get_size(&self) -> StoreResult<u64> {
        Ok(self.blocks.read().await.len() as u64)
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
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send + 'a>>> {
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
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn test_memory_store_put_and_get() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        //================== Raw ==================

        let data = vec![1, 2, 3, 4, 5];
        let cid = store.put_bytes(&data[..]).await?;
        let mut res = store.get_bytes(&cid).await?;

        let mut buf = Vec::new();
        res.read_to_end(&mut buf).await?;

        assert_eq!(data, buf);

        //================= IPLD =================

        let data = fixtures::Directory {
            name: "root".to_string(),
            entries: vec![
                utils::make_cid(Codec::Raw, &[1, 2, 3]),
                utils::make_cid(Codec::Raw, &[4, 5, 6]),
            ],
        };

        let cid = store.put_node(&data).await?;
        let res = store.get_node::<fixtures::Directory>(&cid).await?;

        assert_eq!(res, data);

        Ok(())
    }
}

#[cfg(test)]
mod fixtures {
    use serde::Deserialize;

    use super::*;

    //--------------------------------------------------------------------------------------------------
    // Types
    //--------------------------------------------------------------------------------------------------

    #[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
    pub(super) struct Directory {
        pub(super) name: String,
        pub(super) entries: Vec<Cid>,
    }

    //--------------------------------------------------------------------------------------------------
    // Trait Implementations
    //--------------------------------------------------------------------------------------------------

    impl IpldReferences for Directory {
        fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
            Box::new(self.entries.iter())
        }
    }
}

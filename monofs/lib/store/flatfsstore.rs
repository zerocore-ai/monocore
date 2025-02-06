use std::sync::Arc;
use std::{collections::HashSet, path::PathBuf, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use getset::Getters;
use monoutils::SeekableReader;
use monoutils_store::ipld::codec::Links;
use monoutils_store::{ipld::cid::Cid, FastCDCChunker, FixedSizeChunker};
use monoutils_store::{
    Chunker, Codec, FlatLayout, IpldReferences, IpldStore, IpldStoreSeekable, Layout,
    LayoutSeekable, RawStore, StoreError, StoreResult, DEFAULT_MAX_NODE_BLOCK_SIZE,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_ipld_dagcbor::codec::DagCborCodec;
use tokio::fs::{self, File};
use tokio::io::AsyncRead;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use typed_builder::TypedBuilder;

//--------------------------------------------------------------------------------------------------
// Types: FlatFsStore
//--------------------------------------------------------------------------------------------------

/// The number of directory levels to use for organizing blocks.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DirLevels {
    /// Store all blocks directly in the root directory.
    /// ```text
    /// path/
    /// ├── 0012345...  (full hex-encoded CID digest)
    /// ├── abcdef...   (full hex-encoded CID digest)
    /// └── ffeed...    (full hex-encoded CID digest)
    /// ```
    Zero,

    /// Store blocks in a single level of subdirectories (default).
    /// This is the most common approach used by systems like IPFS and Git.
    /// ```text
    /// path/
    /// ├── 00/
    /// │   └── 0012345...  (full hex-encoded CID digest)
    /// ├── ab/
    /// │   └── abcdef...   (full hex-encoded CID digest)
    /// └── ff/
    ///     └── ffeed...    (full hex-encoded CID digest)
    /// ```
    #[default]
    One,

    /// Store blocks in two levels of subdirectories.
    /// ```text
    /// path/
    /// ├── 00/
    /// │   └── 01/
    /// │       └── 0012345...  (full hex-encoded CID digest)
    /// ├── ab/
    /// │   └── cd/
    /// │       └── abcdef...   (full hex-encoded CID digest)
    /// └── ff/
    ///     └── ee/
    ///         └── ffeed...    (full hex-encoded CID digest)
    /// ```
    Two,
}

/// A flat filesystem store that organizes blocks in a configurable directory structure based on
/// the CID digest.
///
/// The store supports three different directory structures:
/// - Zero levels: All blocks stored directly in the root directory
/// - One level (default): Blocks stored in subdirectories based on the first two characters of the CID digest
/// - Two levels: Blocks stored in nested subdirectories based on the first four characters of the CID digest
///
/// Example directory structure (using one-level organization):
/// ```text
/// store_root/
/// ├── 00/
/// │   ├── 001234567890abcdef...  (block file)
/// │   └── 00fedcba987654321...  (block file)
/// ├── a1/
/// │   └── a1b2c3d4e5f67890...   (block file)
/// └── ff/
///     └── ff0123456789abcd...   (block file)
/// ```
///
/// The default one-level structure is the most common approach, used by systems like IPFS and Git,
/// providing a good balance between directory depth and file distribution (1024 possible subdirectories).
///
/// ## Reference Counting
///
/// The store optionally supports reference counting to track block usage and enable automatic garbage collection.
/// When enabled, each block file stores a reference count that is incremented when the block is referenced by other
/// blocks and decremented during garbage collection. When a block's reference count reaches zero and
/// it is garbage collected, its referenced blocks are also processed recursively. This ensures that
/// blocks are only removed when they are no longer needed, while also cleaning up any unreferenced
/// dependencies.
///
/// Reference counting must be enabled or disabled at store initialization and cannot be changed afterwards.
/// When disabled, blocks are stored without reference counts and garbage collection is not available.
///
/// ## Chunking and Layout
///
/// The store uses a configurable chunking strategy to split data into smaller blocks. The chunker
/// is configurable via the `chunker` field. The layout strategy is configurable via the `layout`
/// field.
#[derive(Debug, Clone, TypedBuilder, Getters)]
#[getset(get = "pub with_prefix")]
pub struct FlatFsStoreImpl<C = FastCDCChunker, L = FlatLayout>
where
    C: Chunker + Default,
    L: Layout + Default,
{
    /// The root path for the store.
    #[builder(setter(into))]
    path: PathBuf,

    /// The number of directory levels to use for organizing blocks.
    #[builder(default)]
    dir_levels: DirLevels,

    /// The chunking algorithm used to split data into chunks.
    #[builder(default)]
    chunker: Arc<C>,

    /// The layout strategy used to store chunked data.
    #[builder(default)]
    layout: Arc<L>,

    /// Whether to enable reference counting for garbage collection.
    #[builder(default = true)]
    enable_refcount: bool,
}

/// A flat filesystem store that organizes blocks in a configurable directory structure based on
/// the CID digest.
///
/// The store supports three different directory structures:
/// - Zero levels: All blocks stored directly in the root directory
/// - One level (default): Blocks stored in subdirectories based on the first two characters of the CID digest
/// - Two levels: Blocks stored in nested subdirectories based on the first four characters of the CID digest
///
/// Example directory structure (using one-level organization):
/// ```text
/// store_root/
/// ├── 00/
/// │   ├── 001234567890abcdef...  (block file)
/// │   └── 00fedcba987654321...  (block file)
/// ├── a1/
/// │   └── a1b2c3d4e5f67890...   (block file)
/// └── ff/
///     └── ff0123456789abcd...   (block file)
/// ```
///
/// The default one-level structure is the most common approach, used by systems like IPFS and Git,
/// providing a good balance between directory depth and file distribution (1024 possible subdirectories).
///
/// ## Reference Counting
///
/// The store optionally supports reference counting to track block usage and enable automatic garbage collection.
/// When enabled, each block file stores a reference count that is incremented when the block is referenced by other
/// blocks and decremented during garbage collection. When a block's reference count reaches zero and
/// it is garbage collected, its referenced blocks are also processed recursively. This ensures that
/// blocks are only removed when they are no longer needed, while also cleaning up any unreferenced
/// dependencies.
///
/// Reference counting must be enabled or disabled at store initialization and cannot be changed afterwards.
/// When disabled, blocks are stored without reference counts and garbage collection is not available.
///
/// ## Chunking and Layout
///
/// This version of the store uses a [`FastCDCChunker`] for chunking and [`FlatLayout`] for layout.
pub type FlatFsStore = FlatFsStoreImpl<FastCDCChunker, FlatLayout>;

/// A [`FlatFsStoreImpl`] with a [`FixedSizeChunker`] for chunking and [`FlatLayout`] for layout.
pub type FlatFsStoreFixed = FlatFsStoreImpl<FixedSizeChunker, FlatLayout>;

//--------------------------------------------------------------------------------------------------
// Methods: FlatFsStore
//--------------------------------------------------------------------------------------------------

impl<C, L> FlatFsStoreImpl<C, L>
where
    C: Chunker + Default,
    L: Layout + Default,
{
    /// Creates a new `FlatFsStore` with the given root path, chunker, and layout.
    /// Reference counting is enabled by default.
    ///
    /// The root path is where all the blocks will be stored. If the path doesn't exist, it will be
    /// created when the first block is stored.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            dir_levels: DirLevels::default(),
            chunker: Default::default(),
            layout: Default::default(),
            enable_refcount: true,
        }
    }

    /// Returns whether reference counting is enabled for this store.
    pub fn is_refcount_enabled(&self) -> bool {
        self.enable_refcount
    }

    /// Get the path for a given CID using the configured directory structure
    fn get_block_path(&self, cid: &Cid) -> PathBuf {
        let digest = hex::encode(cid.hash().digest());
        match self.dir_levels {
            DirLevels::Zero => self.path.join(&digest),
            DirLevels::One => {
                let first = &digest[0..2];
                self.path.join(first).join(&digest)
            }
            DirLevels::Two => {
                let first = &digest[0..2];
                let second = &digest[2..4];
                self.path.join(first).join(second).join(&digest)
            }
        }
    }

    /// Ensure the parent directories exist for a given block path
    async fn ensure_directories(&self, block_path: &PathBuf) -> StoreResult<()> {
        if let Some(parent) = block_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StoreError::custom(e))?;
        }
        Ok(())
    }

    /// Reads the reference count from a file
    async fn read_refcount(&self, file: &mut File) -> StoreResult<u64> {
        let mut refcount_bytes = [0u8; 8];
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(StoreError::custom)?;
        file.read_exact(&mut refcount_bytes)
            .await
            .map_err(StoreError::custom)?;
        Ok(u64::from_be_bytes(refcount_bytes))
    }

    /// Updates the reference count in a file
    async fn write_refcount(&self, file: &mut File, refcount: u64) -> StoreResult<()> {
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(StoreError::custom)?;
        file.write_all(&refcount.to_be_bytes())
            .await
            .map_err(StoreError::custom)?;
        Ok(())
    }

    /// Reads the block data from a file (skipping the refcount if enabled)
    async fn read_block_data(&self, file: &mut File) -> StoreResult<Bytes> {
        if self.enable_refcount {
            file.seek(SeekFrom::Start(8))
                .await
                .map_err(StoreError::custom)?;
        } else {
            file.seek(SeekFrom::Start(0))
                .await
                .map_err(StoreError::custom)?;
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .await
            .map_err(StoreError::custom)?;
        Ok(data.into())
    }

    /// Writes a new block with initial refcount
    async fn write_new_block(&self, block_path: &PathBuf, bytes: &[u8]) -> StoreResult<()> {
        self.ensure_directories(block_path).await?;
        let mut file = File::create(block_path).await.map_err(StoreError::custom)?;

        if self.enable_refcount {
            // Write initial refcount (0)
            file.write_all(&0u64.to_be_bytes())
                .await
                .map_err(StoreError::custom)?;
        }

        // Write block data
        file.write_all(bytes).await.map_err(StoreError::custom)?;
        Ok(())
    }

    /// Increments reference counts for the given CIDs
    async fn increment_reference_counts(
        &self,
        cids: impl Iterator<Item = &Cid>,
    ) -> StoreResult<()> {
        if !self.enable_refcount {
            return Ok(());
        }

        for cid in cids {
            let block_path = self.get_block_path(cid);
            if let Ok(mut file) = File::options()
                .read(true)
                .write(true)
                .open(&block_path)
                .await
            {
                let refcount = self.read_refcount(&mut file).await?;
                self.write_refcount(&mut file, refcount + 1).await?;
            }
        }
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<C, L> IpldStore for FlatFsStoreImpl<C, L>
where
    C: Chunker + Default + Clone + Send + Sync + 'static,
    L: Layout + Default + Clone + Send + Sync + 'static,
{
    async fn put_node<T>(&self, data: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        // Serialize the data to CBOR bytes
        let bytes = serde_ipld_dagcbor::to_vec(&data).map_err(StoreError::custom)?;

        // Check if the data exceeds the node maximum block size
        if let Some(max_size) = self.get_max_node_block_size().await? {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::NodeBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        // Create CID and store the block
        let cid = monoutils_store::generate_cid(Codec::DagCbor, &bytes);
        let block_path = self.get_block_path(&cid);

        if !block_path.exists() {
            self.write_new_block(&block_path, &bytes).await?;
            // Increment reference counts for referenced blocks
            self.increment_reference_counts(data.get_references())
                .await?;
        }

        Ok(cid)
    }

    async fn put_bytes(&self, reader: impl AsyncRead + Send + Sync) -> StoreResult<Cid> {
        tracing::trace!("putting bytes");
        let chunk_stream = self.chunker.chunk(reader).await?;
        let mut cid_stream = self.layout.organize(chunk_stream, self.clone()).await?;

        // Take the last CID from the stream
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
        let block_path = self.get_block_path(cid);
        let mut file = File::open(&block_path)
            .await
            .map_err(|_| StoreError::BlockNotFound(*cid))?;

        let bytes = self.read_block_data(&mut file).await?;
        match cid.codec().try_into()? {
            Codec::DagCbor => serde_ipld_dagcbor::from_slice(&bytes).map_err(StoreError::custom),
            codec => Err(StoreError::UnexpectedBlockCodec(Codec::DagCbor, codec)),
        }
    }

    async fn get_bytes(&self, cid: &Cid) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        self.layout.retrieve(cid, self.clone()).await
    }

    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64> {
        self.layout.get_size(cid, self.clone()).await
    }

    async fn has(&self, cid: &Cid) -> bool {
        self.get_block_path(cid).exists()
    }

    async fn get_supported_codecs(&self) -> HashSet<Codec> {
        let mut codecs = HashSet::new();
        codecs.insert(Codec::Raw);
        codecs.insert(Codec::DagCbor);
        codecs
    }

    async fn get_max_node_block_size(&self) -> StoreResult<Option<u64>> {
        Ok(Some(DEFAULT_MAX_NODE_BLOCK_SIZE))
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        let mut count = 0;
        match self.dir_levels {
            DirLevels::Zero => {
                // Count all files in the root directory
                let mut entries = fs::read_dir(&self.path).await.map_err(StoreError::custom)?;
                while let Some(entry) = entries.next_entry().await.map_err(StoreError::custom)? {
                    if entry
                        .file_type()
                        .await
                        .map_err(StoreError::custom)?
                        .is_file()
                    {
                        count += 1;
                    }
                }
            }
            DirLevels::One => {
                // Count all files in first-level subdirectories
                let mut entries = fs::read_dir(&self.path).await.map_err(StoreError::custom)?;
                while let Some(dir_entry) =
                    entries.next_entry().await.map_err(StoreError::custom)?
                {
                    if dir_entry
                        .file_type()
                        .await
                        .map_err(StoreError::custom)?
                        .is_dir()
                    {
                        let mut subdir_entries = fs::read_dir(dir_entry.path())
                            .await
                            .map_err(StoreError::custom)?;
                        while let Some(file_entry) = subdir_entries
                            .next_entry()
                            .await
                            .map_err(StoreError::custom)?
                        {
                            if file_entry
                                .file_type()
                                .await
                                .map_err(StoreError::custom)?
                                .is_file()
                            {
                                count += 1;
                            }
                        }
                    }
                }
            }
            DirLevels::Two => {
                // Count all files in second-level subdirectories
                let mut entries = fs::read_dir(&self.path).await.map_err(StoreError::custom)?;
                while let Some(l1_entry) = entries.next_entry().await.map_err(StoreError::custom)? {
                    if l1_entry
                        .file_type()
                        .await
                        .map_err(StoreError::custom)?
                        .is_dir()
                    {
                        let mut l2_entries = fs::read_dir(l1_entry.path())
                            .await
                            .map_err(StoreError::custom)?;
                        while let Some(l2_entry) =
                            l2_entries.next_entry().await.map_err(StoreError::custom)?
                        {
                            if l2_entry
                                .file_type()
                                .await
                                .map_err(StoreError::custom)?
                                .is_dir()
                            {
                                let mut file_entries = fs::read_dir(l2_entry.path())
                                    .await
                                    .map_err(StoreError::custom)?;
                                while let Some(file_entry) = file_entries
                                    .next_entry()
                                    .await
                                    .map_err(StoreError::custom)?
                                {
                                    if file_entry
                                        .file_type()
                                        .await
                                        .map_err(StoreError::custom)?
                                        .is_file()
                                    {
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    async fn supports_garbage_collection(&self) -> bool {
        self.enable_refcount
    }

    async fn garbage_collect(&self, cid: &Cid) -> StoreResult<HashSet<Cid>> {
        if !self.enable_refcount {
            return Ok(HashSet::new());
        }

        let mut removed_cids = HashSet::new();
        let block_path = self.get_block_path(cid);

        // Check if the CID exists and has refcount of exactly 0
        let refs = {
            if let Ok(mut file) = File::options()
                .read(true)
                .write(true)
                .open(&block_path)
                .await
            {
                let count = self.read_refcount(&mut file).await?;
                if count == 0 {
                    // Try to deserialize the block to get its references
                    let bytes = self.read_block_data(&mut file).await?;
                    let codec: Codec = cid.codec().try_into()?;

                    // Drop file handle before potential deletion
                    drop(file);

                    match codec {
                        Codec::DagCbor => {
                            // Extract CID references using the Links trait
                            let refs = DagCborCodec::links(&bytes)
                                .map_err(StoreError::custom)?
                                .collect::<Vec<_>>();

                            // Remove the block since refcount is 0
                            fs::remove_file(&block_path)
                                .await
                                .map_err(StoreError::custom)?;
                            removed_cids.insert(*cid);

                            Some(refs)
                        }
                        Codec::Raw => {
                            // For raw blocks, just remove them if refcount is 0
                            fs::remove_file(&block_path)
                                .await
                                .map_err(StoreError::custom)?;
                            removed_cids.insert(*cid);
                            None
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Process dependencies if we had any
        if let Some(refs) = refs {
            for ref_cid in refs {
                let block_path = self.get_block_path(&ref_cid);
                // Decrement refcount and check if we should collect
                let should_collect = {
                    if let Ok(mut file) = File::options()
                        .read(true)
                        .write(true)
                        .open(&block_path)
                        .await
                    {
                        let count = self.read_refcount(&mut file).await?;
                        if count > 0 {
                            self.write_refcount(&mut file, count - 1).await?;
                            count == 1 // Will be 0 after decrement
                        } else {
                            false
                        }
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
impl<C, L> RawStore for FlatFsStoreImpl<C, L>
where
    C: Chunker + Default + Clone + Send + Sync,
    L: Layout + Default + Clone + Send + Sync,
{
    async fn put_raw_block(&self, bytes: impl Into<Bytes> + Send) -> StoreResult<Cid> {
        let bytes = bytes.into();
        if let Some(max_size) = self.get_max_raw_block_size().await? {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::RawBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        let cid = monoutils_store::generate_cid(Codec::Raw, bytes.as_ref());
        let block_path = self.get_block_path(&cid);

        if !block_path.exists() {
            self.write_new_block(&block_path, &bytes).await?;
        }

        Ok(cid)
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        let block_path = self.get_block_path(cid);
        let mut file = File::open(&block_path)
            .await
            .map_err(|_| StoreError::BlockNotFound(*cid))?;

        let bytes = self.read_block_data(&mut file).await?;
        match cid.codec().try_into()? {
            Codec::Raw => Ok(bytes),
            codec => Err(StoreError::UnexpectedBlockCodec(Codec::Raw, codec)),
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
impl<C, L> IpldStoreSeekable for FlatFsStoreImpl<C, L>
where
    C: Chunker + Default + Clone + Send + Sync + 'static,
    L: LayoutSeekable + Default + Clone + Send + Sync + 'static,
{
    async fn get_seekable_bytes(
        &self,
        cid: &Cid,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send + 'static>>> {
        self.layout.retrieve_seekable(cid, self.clone()).await
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::codetable::{Code, MultihashDigest};
    use monoutils_store::{DEFAULT_MAX_CHUNK_SIZE, DEFAULT_MAX_NODE_BLOCK_SIZE};
    use tempfile::TempDir;
    use tokio::fs;

    use super::fixtures::{self, TestNode};
    use super::*;

    #[tokio::test]
    async fn test_flatfsstore_raw_block() -> anyhow::Result<()> {
        for dir_level in [DirLevels::Zero, DirLevels::One, DirLevels::Two] {
            let (store, _temp) = fixtures::setup_store(dir_level).await;

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
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_flatfsstore_bytes() -> anyhow::Result<()> {
        for dir_level in [DirLevels::Zero, DirLevels::One, DirLevels::Two] {
            let (store, _temp) = fixtures::setup_store(dir_level).await;

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
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_flatfsstore_node() -> anyhow::Result<()> {
        for dir_level in [DirLevels::Zero, DirLevels::One, DirLevels::Two] {
            let (store, _temp) = fixtures::setup_store(dir_level).await;

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
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_flatfsstore_directory_structure() -> anyhow::Result<()> {
        for dir_level in [DirLevels::Zero, DirLevels::One, DirLevels::Two] {
            let (store, temp) = fixtures::setup_store(dir_level).await;

            // Store multiple blocks to create directory structure
            let data1 = b"First block".to_vec();
            let data2 = b"Second block".to_vec();
            let data3 = b"Third block".to_vec();

            let cid1 = store.put_raw_block(data1).await?;
            let cid2 = store.put_raw_block(data2).await?;
            let cid3 = store.put_raw_block(data3).await?;

            // Verify directory structure based on level
            match dir_level {
                DirLevels::Zero => {
                    // All files should be in root directory
                    let mut entries = fs::read_dir(temp.path()).await?;
                    let mut count = 0;
                    while let Some(entry) = entries.next_entry().await? {
                        assert!(entry.file_type().await?.is_file());
                        count += 1;
                    }
                    assert_eq!(count, 3);
                }
                DirLevels::One => {
                    // Files should be in first-level directories
                    for cid in [&cid1, &cid2, &cid3] {
                        let digest = hex::encode(cid.hash().digest());
                        let first = &digest[0..2];
                        assert!(temp.path().join(first).is_dir());
                    }
                }
                DirLevels::Two => {
                    // Files should be in second-level directories
                    for cid in [&cid1, &cid2, &cid3] {
                        let digest = hex::encode(cid.hash().digest());
                        let first = &digest[0..2];
                        let second = &digest[2..4];
                        assert!(temp.path().join(first).join(second).is_dir());
                    }
                }
            }

            // Verify block count
            assert_eq!(store.get_block_count().await?, 3);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_flatfsstore_error_handling() -> anyhow::Result<()> {
        let (store, _temp) = fixtures::setup_store(DirLevels::One).await;

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
    async fn test_flatfsstore_operations() -> anyhow::Result<()> {
        let (store, _temp) = fixtures::setup_store(DirLevels::One).await;

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
    async fn test_flatfsstore_garbage_collect() -> anyhow::Result<()> {
        let (store, _temp) = fixtures::setup_store(DirLevels::One).await;

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

        // Read back the node to verify refs are properly serialized
        let retrieved_node: TestNode = store.get_node(&node_cid).await?;
        assert_eq!(retrieved_node.refs, vec![data_cid]);

        // Try to garbage collect data_cid - should fail because node references it
        let removed = store.garbage_collect(&data_cid).await?;
        assert!(removed.is_empty());
        assert!(store.has(&data_cid).await);

        // Garbage collect node_cid - should succeed and trigger data_cid collection
        let removed = store.garbage_collect(&node_cid).await?;
        assert!(removed.contains(&node_cid));
        assert!(removed.contains(&data_cid));
        assert!(!store.has(&node_cid).await);
        assert!(!store.has(&data_cid).await);
        assert!(store.is_empty().await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_flatfsstore_complex_garbage_collect() -> anyhow::Result<()> {
        let (store, _temp) = fixtures::setup_store(DirLevels::One).await;

        // Create some raw data blocks
        let data1 = b"Data block 1".to_vec();
        let data2 = b"Data block 2".to_vec();
        let data3 = b"Data block 3".to_vec();
        let data1_cid = store.put_raw_block(data1.clone()).await?;
        let data2_cid = store.put_raw_block(data2.clone()).await?;
        let data3_cid = store.put_raw_block(data3.clone()).await?;

        // ======================== Tree Structure ========================
        //
        //             root[val: 5, rc: 0]
        //                 /            \
        //                /              \
        //               /                \
        //              /                  \
        //             /                    \
        // middle1[val: 3, rc: 1]      middle2[val: 4, rc: 1]
        //        /            \                /          \
        //       /              \              /            \
        //      /                \            /              \
        //     /                  \          /                \
        //    /                    \        /                  \
        // leaf1[val: 1, rc: 1]  leaf2[val: 2, rc: 2]           \
        //     |                           |                     \
        //     |                           |                      \
        //     |                           |                       \
        //     |                           |                        \
        // data1[rc: 1]             data2[rc: 1]             data3[rc: 1]
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

        // Try to collect leaf1 - should fail as it's referenced by middle1
        let removed = store.garbage_collect(&leaf1_cid).await?;
        assert!(
            removed.is_empty(),
            "No nodes should be removed when collecting referenced leaf1"
        );
        assert!(store.has(&leaf1_cid).await);
        assert!(store.has(&data1_cid).await);

        // Try to collect middle1 - should fail as it's referenced by root
        let removed = store.garbage_collect(&middle1_cid).await?;
        assert!(
            removed.is_empty(),
            "No nodes should be removed when collecting referenced middle1"
        );
        assert!(store.has(&middle1_cid).await);
        assert!(store.has(&leaf1_cid).await);
        assert!(store.has(&leaf2_cid).await);

        // Try to collect data2 - should fail as it's referenced by leaf2
        let removed = store.garbage_collect(&data2_cid).await?;
        assert!(
            removed.is_empty(),
            "No nodes should be removed when collecting referenced data2"
        );
        assert!(store.has(&data2_cid).await);

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

        Ok(())
    }

    #[tokio::test]
    async fn test_flatfsstore_disabled_refcount() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let store = FlatFsStore::builder()
            .path(temp_dir.path())
            .dir_levels(DirLevels::default())
            .enable_refcount(false)
            .build();

        // Store some test data
        let data = b"Hello, World!".to_vec();
        let cid = store.put_raw_block(data.clone()).await?;
        let block_path = store.get_block_path(&cid);

        // Verify the data was written correctly
        let file_contents = fs::read(&block_path).await?;
        assert_eq!(
            &file_contents,
            data.as_slice(),
            "File should contain only raw data without refcount header"
        );
        assert_eq!(
            file_contents.len(),
            data.len(),
            "File size should match data size exactly"
        );

        // Verify we can read it back through the store API
        let retrieved = store.get_raw_block(&cid).await?;
        assert_eq!(retrieved.as_ref(), data.as_slice());

        // Verify garbage collection is disabled
        assert!(!store.supports_garbage_collection().await);
        let removed = store.garbage_collect(&cid).await?;
        assert!(removed.is_empty(), "Garbage collection should be no-op");
        assert!(
            store.has(&cid).await,
            "Block should still exist after gc attempt"
        );

        // Test with a node containing references
        let node = TestNode {
            name: "test".to_string(),
            value: 42,
            refs: vec![cid],
        };
        let node_cid = store.put_node(&node).await?;
        let node_path = store.get_block_path(&node_cid);

        // Read the raw node file contents
        let node_file_contents = fs::read(&node_path).await?;

        // Deserialize using CBOR to verify the data
        let stored_node: TestNode = serde_ipld_dagcbor::from_slice(&node_file_contents)?;
        assert_eq!(stored_node, node, "Node data should be preserved");

        // Verify the node can still reference other blocks without refcounting
        let retrieved_node: TestNode = store.get_node(&node_cid).await?;
        assert_eq!(retrieved_node.refs, vec![cid]);

        // Try to read the referenced block
        let referenced_data = store.get_raw_block(&cid).await?;
        assert_eq!(referenced_data.as_ref(), data.as_slice());

        Ok(())
    }
}

#[cfg(test)]
mod fixtures {
    use serde::Deserialize;
    use tempfile::TempDir;

    use super::*;

    // Helper function to create a store with a temporary directory
    pub(super) async fn setup_store(dir_levels: DirLevels) -> (FlatFsStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = FlatFsStore::builder()
            .dir_levels(dir_levels)
            .path(temp_dir.path())
            .build();
        (store, temp_dir)
    }

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

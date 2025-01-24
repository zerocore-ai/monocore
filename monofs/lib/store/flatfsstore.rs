use std::{collections::HashSet, fs, future::Future, path::PathBuf, pin::Pin};

use bytes::Bytes;
use futures::StreamExt;
use monoutils::SeekableReader;
use monoutils_store::{
    codetable::{Code, MultihashDigest},
    ipld::cid::Cid,
    FastCDCChunker, FixedSizeChunker,
};
use monoutils_store::{
    Chunker, Codec, FlatLayout, IpldReferences, IpldStore, IpldStoreSeekable, Layout,
    LayoutSeekable, RawStore, StoreError, StoreResult,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncRead;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
#[derive(Debug, Clone)]
pub struct FlatFsStore<C = FastCDCChunker, L = FlatLayout>
where
    C: Chunker,
    L: Layout,
{
    path: PathBuf,
    dir_levels: DirLevels,
    chunker: C,
    layout: L,
}

/// A [`FlatFsStore`] with a [`FastCDCChunker`] and [`FlatLayout`].
pub type FlatFsStoreDefault = FlatFsStore<FastCDCChunker, FlatLayout>;

/// A [`FlatFsStore`] with a [`FixedSizeChunker`] and [`FlatLayout`].
pub type FlatFsStoreFixed = FlatFsStore<FixedSizeChunker, FlatLayout>;

//--------------------------------------------------------------------------------------------------
// Methods: FlatFsStore
//--------------------------------------------------------------------------------------------------

impl<C, L> FlatFsStore<C, L>
where
    C: Chunker,
    L: Layout,
{
    /// Creates a new `FlatFsStore` with the given root path, chunker, and layout.
    ///
    /// The root path is where all the blocks will be stored. If the path doesn't exist, it will be
    /// created when the first block is stored.
    pub fn new(path: impl AsRef<str>) -> Self
    where
        C: Default,
        L: Default,
    {
        Self {
            path: PathBuf::from(path.as_ref()),
            dir_levels: DirLevels::default(),
            chunker: Default::default(),
            layout: Default::default(),
        }
    }

    /// Creates a new `FlatFsStore` with the given root path, chunker, and layout.
    pub fn with_chunker_and_layout(path: impl AsRef<str>, chunker: C, layout: L) -> Self {
        Self {
            path: PathBuf::from(path.as_ref()),
            dir_levels: DirLevels::default(),
            chunker,
            layout,
        }
    }

    /// Creates a new `FlatFsStore` with the given root path and default chunker and layout.
    pub fn with_dir_levels(path: impl AsRef<str>, dir_levels: DirLevels) -> Self
    where
        C: Default,
        L: Default,
    {
        Self {
            path: PathBuf::from(path.as_ref()),
            dir_levels,
            chunker: Default::default(),
            layout: Default::default(),
        }
    }

    /// Creates a new `FlatFsStore` with the given root path, directory structure, chunker, and layout.
    pub fn with_dir_levels_chunker_and_layout(
        path: impl AsRef<str>,
        dir_levels: DirLevels,
        chunker: C,
        layout: L,
    ) -> Self {
        Self {
            path: PathBuf::from(path.as_ref()),
            dir_levels,
            chunker,
            layout,
        }
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
            create_dir_all(parent)
                .await
                .map_err(|e| StoreError::custom(e))?;
        }
        Ok(())
    }

    /// Create a CID for the given bytes with the specified codec
    fn make_cid(&self, codec: Codec, bytes: &[u8]) -> Cid {
        let digest = Code::Blake3_256.digest(bytes);
        Cid::new_v1(codec.into(), digest)
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<C, L> IpldStore for FlatFsStore<C, L>
where
    C: Chunker + Clone + Send + Sync,
    L: Layout + Clone + Send + Sync,
{
    async fn put_node<T>(&self, data: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        // Serialize the data to CBOR bytes
        let bytes = serde_ipld_dagcbor::to_vec(&data).map_err(StoreError::custom)?;

        // Check if the data exceeds the node maximum block size
        if let Some(max_size) = self.get_node_block_max_size() {
            if bytes.len() as u64 > max_size {
                return Err(StoreError::NodeBlockTooLarge(bytes.len() as u64, max_size));
            }
        }

        // Create CID and store the block
        let cid = self.make_cid(Codec::DagCbor, &bytes);
        let block_path = self.get_block_path(&cid);

        self.ensure_directories(&block_path).await?;
        let mut file = File::create(&block_path)
            .await
            .map_err(StoreError::custom)?;
        file.write_all(&bytes).await.map_err(StoreError::custom)?;

        Ok(cid)
    }

    async fn put_bytes<'a>(
        &'a self,
        reader: impl AsyncRead + Send + Sync + 'a,
    ) -> StoreResult<Cid> {
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
        let bytes = self.get_raw_block(cid).await?;
        match cid.codec().try_into()? {
            Codec::DagCbor => serde_ipld_dagcbor::from_slice(&bytes).map_err(StoreError::custom),
            codec => Err(StoreError::UnexpectedBlockCodec(Codec::DagCbor, codec)),
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

    async fn has(&self, cid: &Cid) -> bool {
        self.get_block_path(cid).exists()
    }

    fn get_supported_codecs(&self) -> HashSet<Codec> {
        let mut codecs = HashSet::new();
        codecs.insert(Codec::Raw);
        codecs.insert(Codec::DagCbor);
        codecs
    }

    fn get_node_block_max_size(&self) -> Option<u64> {
        self.chunker.chunk_max_size()
    }

    async fn get_block_count(&self) -> StoreResult<u64> {
        let mut count = 0;
        match self.dir_levels {
            DirLevels::Zero => {
                // Count all files in the root directory
                let entries = fs::read_dir(&self.path).map_err(StoreError::custom)?;
                for entry in entries {
                    let entry = entry.map_err(StoreError::custom)?;
                    if entry.file_type().map_err(StoreError::custom)?.is_file() {
                        count += 1;
                    }
                }
            }
            DirLevels::One => {
                // Count all files in first-level subdirectories
                let entries = fs::read_dir(&self.path).map_err(StoreError::custom)?;
                for dir_entry in entries {
                    let dir_entry = dir_entry.map_err(StoreError::custom)?;
                    if dir_entry.file_type().map_err(StoreError::custom)?.is_dir() {
                        let subdir_entries =
                            fs::read_dir(dir_entry.path()).map_err(StoreError::custom)?;
                        for file_entry in subdir_entries {
                            let file_entry = file_entry.map_err(StoreError::custom)?;
                            if file_entry
                                .file_type()
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
                let entries = fs::read_dir(&self.path).map_err(StoreError::custom)?;
                for l1_entry in entries {
                    let l1_entry = l1_entry.map_err(StoreError::custom)?;
                    if l1_entry.file_type().map_err(StoreError::custom)?.is_dir() {
                        let l2_entries =
                            fs::read_dir(l1_entry.path()).map_err(StoreError::custom)?;
                        for l2_entry in l2_entries {
                            let l2_entry = l2_entry.map_err(StoreError::custom)?;
                            if l2_entry.file_type().map_err(StoreError::custom)?.is_dir() {
                                let file_entries =
                                    fs::read_dir(l2_entry.path()).map_err(StoreError::custom)?;
                                for file_entry in file_entries {
                                    let file_entry = file_entry.map_err(StoreError::custom)?;
                                    if file_entry
                                        .file_type()
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
}

impl<C, L> RawStore for FlatFsStore<C, L>
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

        let cid = self.make_cid(Codec::Raw, bytes.as_ref());
        let block_path = self.get_block_path(&cid);

        self.ensure_directories(&block_path).await?;
        let mut file = File::create(&block_path)
            .await
            .map_err(StoreError::custom)?;
        file.write_all(&bytes).await.map_err(StoreError::custom)?;

        Ok(cid)
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        let block_path = self.get_block_path(cid);
        let mut file = File::open(&block_path)
            .await
            .map_err(|_| StoreError::BlockNotFound(*cid))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .await
            .map_err(StoreError::custom)?;
        Ok(bytes.into())
    }

    fn get_raw_block_max_size(&self) -> Option<u64> {
        self.chunker.chunk_max_size()
    }
}

impl<C, L> IpldStoreSeekable for FlatFsStore<C, L>
where
    C: Chunker + Clone + Send + Sync,
    L: LayoutSeekable + Clone + Send + Sync,
{
    fn get_seekable_bytes<'a>(
        &'a self,
        cid: &'a Cid,
    ) -> impl Future<Output = StoreResult<Pin<Box<dyn SeekableReader + Send + Sync + 'a>>>> + Send
    {
        self.layout.retrieve_seekable(cid, self.clone())
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::DEFAULT_MAX_CHUNK_SIZE;
    use std::fs;
    use tokio::io::AsyncReadExt;

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
                    let entries: Vec<_> = fs::read_dir(temp.path())?.collect();
                    assert_eq!(entries.len(), 3);
                    for entry in entries {
                        assert!(entry?.file_type()?.is_file());
                    }
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
    use tempfile::TempDir;

    use super::*;

    // Helper function to create a store with a temporary directory
    pub(super) async fn setup_store(dir_levels: DirLevels) -> (FlatFsStoreDefault, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = FlatFsStore::with_dir_levels(temp_dir.path().to_str().unwrap(), dir_levels);
        (store, temp_dir)
    }

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

use std::{collections::HashSet, pin::Pin};

use async_trait::async_trait;
use bytes::Bytes;
use getset::Getters;
use ipld_core::cid::Cid;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncRead;

use crate::{Codec, IpldReferences, IpldStore, RawStore, StoreError, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A store that combines two IPLD stores, allowing configurable read and write operations between them.
///
/// `DualStore` provides a way to use two different IPLD stores together, with configurable policies for
/// reading and writing data. This can be useful in several scenarios:
///
/// - Implementing a caching layer where writes go to a fast store and reads check the cache first
/// - Migrating data between stores where writes go to the new store but reads check both
/// - Creating a hybrid store that combines the benefits of two different storage backends
///
/// ## Examples
///
/// ```
/// use monoutils_store::{MemoryStore, DualStore, DualStoreConfig, Choice};
///
/// // Create two stores
/// let store_a = MemoryStore::default();
/// let store_b = MemoryStore::default();
///
/// // Configure to read from A first, write to B
/// let config = DualStoreConfig {
///     read_from: Choice::A,
///     write_to: Choice::B,
/// };
///
/// let dual_store = DualStore::new(store_a, store_b, config);
/// ```
#[derive(Debug, Clone, Getters)]
#[getset(get = "pub with_prefix")]
pub struct DualStore<A, B>
where
    A: IpldStore,
    B: IpldStore,
{
    /// The first store.
    store_a: A,

    /// The second store.
    store_b: B,

    /// The configuration for the dual store.
    config: DualStoreConfig,
}

/// Specifies which store to use for operations in a `DualStore`.
///
/// This enum is used to configure the read and write behavior of a `DualStore`.
/// It allows for flexible routing of operations between the two underlying stores.
///
/// ## Examples
///
/// ```
/// use monoutils_store::{DualStoreConfig, Choice};
///
/// // Configure to read from store A and write to store B
/// let config = DualStoreConfig {
///     read_from: Choice::A,
///     write_to: Choice::B,
/// };
///
/// // Configure to use store A for both reads and writes
/// let config = DualStoreConfig {
///     read_from: Choice::A,
///     write_to: Choice::A,
/// };
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    /// Use the first store (store A)
    A,
    /// Use the second store (store B)
    B,
}

/// Configuration for a `DualStore` that specifies which store to use for reads and writes.
///
/// This configuration allows for flexible routing of operations between the two stores:
/// - `read_from`: Determines which store to check first when reading data
/// - `write_to`: Determines which store new data should be written to
///
/// When reading data, if the data is not found in the primary store specified by `read_from`,
/// the other store will be checked as a fallback.
///
/// By default, both `read_from` and `write_to` are set to `Choice::A`, meaning all operations
/// will primarily use the first store. This provides a simple single-store behavior out of the box,
/// while still allowing for fallback reads from the second store.
///
/// ## Examples
///
/// ```
/// use monoutils_store::{DualStoreConfig, Choice};
///
/// // Default configuration - use store A for both reads and writes
/// let default_config = DualStoreConfig::default();
/// assert_eq!(default_config.read_from, Choice::A);
/// assert_eq!(default_config.write_to, Choice::A);
///
/// // Create a cache-like configuration where:
/// // - Reads check the fast store (A) first, then fallback to the slow store (B)
/// // - Writes go to the fast store (A)
/// let cache_config = DualStoreConfig {
///     read_from: Choice::A,
///     write_to: Choice::A,
/// };
///
/// // Create a migration configuration where:
/// // - Reads check both stores (starting with A)
/// // - Writes go only to the new store (B)
/// let migration_config = DualStoreConfig {
///     read_from: Choice::A,
///     write_to: Choice::B,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct DualStoreConfig {
    /// The store to write to.
    pub write_to: Choice,

    /// The store to read from first.
    pub read_from: Choice,
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
    async fn get_node_in<D>(&self, cid: &Cid, choice: Choice) -> StoreResult<D>
    where
        D: DeserializeOwned + Send,
    {
        match choice {
            Choice::A => self.store_a.get_node(cid).await,
            Choice::B => self.store_b.get_node(cid).await,
        }
    }

    /// Gets the bytes stored in a chosen store as raw bytes by its `Cid`.
    async fn get_bytes_in(
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
    async fn get_bytes_size_from(&self, cid: &Cid, choice: Choice) -> StoreResult<u64> {
        match choice {
            Choice::A => self.store_a.get_bytes_size(cid).await,
            Choice::B => self.store_b.get_bytes_size(cid).await,
        }
    }

    /// Gets raw bytes from a chosen store as a single block by its `Cid`.
    async fn get_raw_block_from(&self, cid: &Cid, choice: Choice) -> StoreResult<Bytes> {
        match choice {
            Choice::A => self.store_a.get_raw_block(cid).await,
            Choice::B => self.store_b.get_raw_block(cid).await,
        }
    }

    /// Saves a serializable type to a chosen store and returns the `Cid` to it.
    async fn put_node_into<T>(&self, data: &T, choice: Choice) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        match choice {
            Choice::A => self.store_a.put_node(data).await,
            Choice::B => self.store_b.put_node(data).await,
        }
    }

    /// Saves raw bytes to a chosen store and returns the `Cid` to it.
    async fn put_bytes_into(
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
    async fn put_raw_block_into(
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
    async fn has_in(&self, cid: &Cid, choice: Choice) -> bool {
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
    async fn put_node<T>(&self, node: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences + Sync,
    {
        self.put_node_into(node, self.config.write_to).await
    }

    async fn put_bytes(&self, bytes: impl AsyncRead + Send + Sync) -> StoreResult<Cid> {
        self.put_bytes_into(bytes, self.config.write_to).await
    }

    async fn get_node<D>(&self, cid: &Cid) -> StoreResult<D>
    where
        D: DeserializeOwned + Send,
    {
        match self.get_node_in(cid, self.config.read_from).await {
            Ok(data) => Ok(data),
            Err(StoreError::BlockNotFound(_)) => {
                let choice = self.config.read_from.other();
                self.get_node_in(cid, choice).await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_bytes(&self, cid: &Cid) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        match self.get_bytes_in(cid, self.config.read_from).await {
            Ok(bytes) => Ok(bytes),
            Err(StoreError::BlockNotFound(_)) => {
                let choice = self.config.read_from.other();
                self.get_bytes_in(cid, choice).await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_bytes_size(&self, cid: &Cid) -> StoreResult<u64> {
        self.get_bytes_size_from(cid, self.config.read_from).await
    }

    async fn has(&self, cid: &Cid) -> bool {
        match self.has_in(cid, self.config.read_from).await {
            true => true,
            false => self.has_in(cid, self.config.read_from.other()).await,
        }
    }

    async fn get_supported_codecs(&self) -> HashSet<Codec> {
        let mut codecs = self.store_a.get_supported_codecs().await;
        codecs.extend(self.store_b.get_supported_codecs().await);
        codecs
    }

    async fn get_max_node_block_size(&self) -> StoreResult<Option<u64>> {
        let max_size_a = self.store_a.get_max_node_block_size().await?;
        let max_size_b = self.store_b.get_max_node_block_size().await?;
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
        self.put_raw_block_into(bytes, self.config.write_to).await
    }

    async fn get_raw_block(&self, cid: &Cid) -> StoreResult<Bytes> {
        match self.get_raw_block_from(cid, self.config.read_from).await {
            Ok(bytes) => Ok(bytes),
            Err(StoreError::BlockNotFound(_)) => {
                let choice = self.config.read_from.other();
                self.get_raw_block_from(cid, choice).await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_max_raw_block_size(&self) -> StoreResult<Option<u64>> {
        let max_size_a = self.store_a.get_max_raw_block_size().await?;
        let max_size_b = self.store_b.get_max_raw_block_size().await?;
        Ok(max_size_a.max(max_size_b))
    }
}

impl Default for DualStoreConfig {
    fn default() -> Self {
        Self {
            write_to: Choice::A,
            read_from: Choice::A,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use crate::MemoryStore;

    use super::*;

    #[tokio::test]
    async fn test_dual_store_default_config() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();
        let dual_store = DualStore::new(store_a.clone(), store_b.clone(), Default::default());

        // Test that data is written to store A by default
        let cid = dual_store.put_node(&"test data").await?;
        assert!(store_a.has(&cid).await);
        assert!(!store_b.has(&cid).await);

        // Test that data is read from store A by default
        assert_eq!(dual_store.get_node::<String>(&cid).await?, "test data");

        Ok(())
    }

    #[tokio::test]
    async fn test_dual_store_basic_operations() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();
        let dual_store = DualStore::new(store_a.clone(), store_b.clone(), Default::default());

        // Test putting and getting data
        let cid = dual_store.put_node(&"test data").await?;
        assert_eq!(dual_store.get_node::<String>(&cid).await?, "test data");

        // Verify data is in store A (default write_to) but not in store B
        assert!(store_a.has(&cid).await);
        assert!(!store_b.has(&cid).await);

        Ok(())
    }

    #[tokio::test]
    async fn test_dual_store_read_fallback() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();

        // Configure to read from A first, write to B
        let config = DualStoreConfig {
            read_from: Choice::A,
            write_to: Choice::B,
        };
        let dual_store = DualStore::new(store_a.clone(), store_b.clone(), config);

        // Write data - should go to store B
        let cid = dual_store.put_node(&"fallback test").await?;

        // Data should be readable even though it's not in the primary read store
        assert_eq!(dual_store.get_node::<String>(&cid).await?, "fallback test");

        Ok(())
    }

    #[tokio::test]
    async fn test_dual_store_different_configurations() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();

        // Test writing to A, reading from B
        let config_1 = DualStoreConfig {
            read_from: Choice::B,
            write_to: Choice::A,
        };
        let dual_store_1 = DualStore::new(store_a.clone(), store_b.clone(), config_1);

        // Test writing to B, reading from A
        let config_2 = DualStoreConfig {
            read_from: Choice::A,
            write_to: Choice::B,
        };
        let dual_store_2 = DualStore::new(store_a.clone(), store_b.clone(), config_2);

        // Write data using both configurations
        let cid_1 = dual_store_1.put_node(&"data 1").await?;
        let cid_2 = dual_store_2.put_node(&"data 2").await?;

        // Verify data location
        assert!(store_a.has(&cid_1).await);
        assert!(store_b.has(&cid_2).await);

        // Both should be readable from either dual store
        assert_eq!(dual_store_1.get_node::<String>(&cid_1).await?, "data 1");
        assert_eq!(dual_store_1.get_node::<String>(&cid_2).await?, "data 2");
        assert_eq!(dual_store_2.get_node::<String>(&cid_1).await?, "data 1");
        assert_eq!(dual_store_2.get_node::<String>(&cid_2).await?, "data 2");

        Ok(())
    }

    #[tokio::test]
    async fn test_dual_store_raw_operations() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();
        let dual_store = DualStore::new(store_a.clone(), store_b.clone(), Default::default());

        // Test raw block operations
        let data = Bytes::from("raw test data");
        let cid = dual_store.put_raw_block(data.clone()).await?;

        let retrieved = dual_store.get_raw_block(&cid).await?;
        assert_eq!(retrieved, data);

        Ok(())
    }

    #[tokio::test]
    async fn test_dual_store_bytes_operations() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();
        let dual_store = DualStore::new(store_a.clone(), store_b.clone(), Default::default());

        // Create test data
        let test_data = b"test bytes data".to_vec();
        let reader = std::io::Cursor::new(test_data.clone());

        // Store the data
        let cid = dual_store.put_bytes(reader).await?;

        // Read the data back
        let mut retrieved_data = Vec::new();
        let mut reader = dual_store.get_bytes(&cid).await?;
        reader.read_to_end(&mut retrieved_data).await?;

        assert_eq!(retrieved_data, test_data);

        // Check size
        let size = dual_store.get_bytes_size(&cid).await?;
        assert_eq!(size as usize, test_data.len());

        Ok(())
    }

    #[tokio::test]
    async fn test_dual_store_metadata() -> anyhow::Result<()> {
        let store_a = MemoryStore::default();
        let store_b = MemoryStore::default();
        let dual_store = DualStore::new(store_a.clone(), store_b.clone(), Default::default());

        // Test initial state
        assert!(dual_store.is_empty().await?);
        assert_eq!(dual_store.get_block_count().await?, 0);

        // Add some data
        dual_store.put_node(&"test data 1").await?;
        dual_store.put_node(&"test data 2").await?;

        // Check updated state
        assert!(!dual_store.is_empty().await?);
        assert_eq!(dual_store.get_block_count().await?, 2);

        // Check supported codecs
        let codecs = dual_store.get_supported_codecs().await;
        assert!(!codecs.is_empty());

        Ok(())
    }
}

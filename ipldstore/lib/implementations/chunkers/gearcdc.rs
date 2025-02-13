use std::{pin::pin, sync::Arc};

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{Chunker, StoreError, StoreResult, DEFAULT_DESIRED_CHUNK_SIZE, DEFAULT_GEAR_TABLE};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A content-defined chunking (CDC) implementation that uses a gear-based rolling hash to identify
/// chunk boundaries.
///
/// CDC is a method of splitting data into chunks based on its content rather than fixed positions.
/// This results in better deduplication as insertions or deletions only affect nearby chunks rather
/// than shifting all subsequent chunk boundaries.
///
/// The gear-based CDC algorithm works by:
/// 1. Computing a rolling hash over a sliding window of bytes
/// 2. Using the gear table to generate pseudo-random values for each byte
/// 3. Declaring a chunk boundary when the hash meets certain criteria (specific bits are zero)
///
/// The average chunk size is controlled by the `desired_chunk_size` parameter, though actual
/// chunk sizes will vary based on content.
#[derive(Clone, Debug)]
pub struct GearCDCChunker {
    /// The gear table used to generate pseudo-random values for each byte.
    /// Each byte maps to a 64-bit value that contributes to the rolling hash.
    gear_table: Arc<[u64; 256]>,

    /// The target average chunk size.
    /// The actual chunk size will vary based on content, but will average around this value.
    desired_chunk_size: u64,
}

/// A rolling hash implementation used by [`GearCDCChunker`] to identify chunk boundaries.
///
/// The gear hash maintains a running hash value that is efficiently updated as new bytes
/// are processed. It uses a pre-computed gear table to map each input byte to a pseudo-random
/// value, which helps ensure an even distribution of chunk boundaries.
///
/// The hash is updated using three components:
/// 1. Left-shifting the current hash by 1 bit (`hash << 1`)
/// 2. XORing with the gear table value for the new byte
/// 3. XORing with the top 11 bits of the hash (`hash >> 53`)
///
/// The third component is crucial for handling real-world data:
/// - For random data, even a simple rolling hash (just components 1 and 2) would work well
/// - However, real data often contains repetitive patterns (e.g., repeated HTML tags, log lines)
/// - The feedback from high bits (component 3) provides additional mixing that helps break up
///   these patterns, ensuring we still get reasonable chunk boundaries even with repetitive data
///
/// This implementation ensures robust chunking behavior for both random and non-random data,
/// which is essential for effective content-defined chunking in real-world applications.
#[derive(Clone, Debug)]
pub struct GearHasher {
    /// The gear table maps each possible byte value to a pseudo-random 64-bit number.
    /// This helps ensure an even distribution of hash values.
    gear_table: Arc<[u64; 256]>,

    /// The current hash value, updated as new bytes are processed.
    /// The hash update includes feedback from high bits to ensure good mixing
    /// even with repetitive data patterns.
    hash: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl GearCDCChunker {
    /// Creates a new `GearCDCChunker` with the given `desired_chunk_size`.
    pub fn new(desired_chunk_size: u64, gear_table: [u64; 256]) -> Self {
        Self {
            gear_table: Arc::new(gear_table),
            desired_chunk_size,
        }
    }

    /// Converts a desired chunk size to a bit mask where the lowest log2(size) bits are set to 1.
    /// This mask is used to determine chunk boundaries in content-defined chunking.
    ///
    /// For example:
    /// - If size is 8192 (2^13), returns a mask with 13 lowest bits set: 0x1FFF
    /// - If size is 16384 (2^14), returns a mask with 14 lowest bits set: 0x3FFF
    ///
    /// # Panics
    /// Panics if size is 0 or greater than 2^63 (as it would exceed u64 capacity)
    pub fn size_to_mask(size: u64) -> u64 {
        assert!(
            size > 0 && size <= (1 << 63),
            "size must be between 1 and 2^63"
        );

        // If size == 1, the doc/tests want 1 bit => 0b1 = 1, even though log2(1) = 0.
        if size == 1 {
            return 0b1;
        }

        // Round up to next power of two
        let p = size.next_power_of_two(); // e.g. 7 -> 8, 9 -> 16, etc.
        let bits = p.trailing_zeros(); // number of bits = log2(p)
        (1 << bits) - 1
    }
}

impl GearHasher {
    /// Creates a new `GearHasher` with the given `desired_chunk_size`.
    pub fn new(gear_table: Arc<[u64; 256]>) -> Self {
        Self {
            gear_table,
            hash: 0,
        }
    }

    /// Updates the rolling hash with a new byte.
    ///
    /// The update process combines three operations:
    /// 1. `hash << 1`: Shifts existing hash left, making room for new information
    /// 2. `^ gear_table[byte]`: Incorporates the new byte's pseudo-random value
    /// 3. `^ (hash >> 53)`: Feeds back high bits for better mixing
    ///
    /// Visually, the process looks like this:
    /// ```text
    /// Original hash:
    /// ┌─────────────────────────────────────────┐
    /// │ bits [63 .. 0]                          │
    /// └─────────────────────────────────────────┘
    ///
    /// After left shift (hash << 1):
    /// ┌─────────────────────────────────────────┐ (the left bit is gone,
    /// │ bits [62 .. 0] 0                        │  the right is 0)
    /// └─────────────────────────────────────────┘
    ///
    /// High bits feedback (hash >> 53):
    /// ┌─────────────────────────────────────────┐
    /// │ bits [63 .. 53] (rest are 0)            │ (~the top 11 bits)
    /// └─────────────────────────────────────────┘
    /// ```
    ///
    /// The feedback from high bits (component 3) is particularly important for handling
    /// repetitive data patterns. Without it, repeated sequences might not generate
    /// enough variation in the lower bits to create chunk boundaries at the desired
    /// frequency. This extra mixing ensures robust chunking even with non-random data.
    #[inline]
    pub fn roll(&mut self, byte: u8) {
        self.hash = (self.hash << 1) ^ self.gear_table[byte as usize] ^ (self.hash >> 53);
    }

    /// Returns the current hash value
    pub fn fingerprint(&self) -> u64 {
        self.hash
    }

    /// Checks if the current hash indicates a chunk boundary
    /// A chunk boundary is determined by checking if the lowest bits of the hash are all zeros
    #[inline]
    pub fn boundary_check(&self, mask: u64) -> bool {
        (self.hash & mask) == 0
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl Chunker for GearCDCChunker {
    async fn chunk(
        &self,
        reader: impl AsyncRead + Send + Sync + 'life0,
    ) -> StoreResult<BoxStream<'_, StoreResult<Bytes>>> {
        let mask = Self::size_to_mask(self.desired_chunk_size);
        let gear_table = self.gear_table.clone();

        let s = try_stream! {
            let mut reader = pin!(reader);
            let mut current_chunk = Vec::new();
            let mut hasher = GearHasher::new(gear_table);
            let mut buffer = [0u8; 8192]; // Read in 8KB chunks

            loop {
                let n = reader.read(&mut buffer).await.map_err(StoreError::custom)?;
                if n == 0 {
                    // End of input - yield remaining bytes as final chunk if any
                    if !current_chunk.is_empty() {
                        yield Bytes::from(current_chunk);
                    }
                    break;
                }

                // Process each byte, looking for chunk boundaries
                for &byte in &buffer[..n] {
                    current_chunk.push(byte);
                    hasher.roll(byte);

                    // Check if we've hit a chunk boundary
                    if hasher.boundary_check(mask) && !current_chunk.is_empty() {
                        yield Bytes::from(current_chunk);
                        current_chunk = Vec::new();
                    }
                }
            }
        };

        Ok(Box::pin(s))
    }

    async fn chunk_max_size(&self) -> StoreResult<Option<u64>> {
        Ok(None) // Variable-size chunks don't have a fixed maximum size
    }
}

impl Default for GearCDCChunker {
    fn default() -> Self {
        Self::new(DEFAULT_DESIRED_CHUNK_SIZE, DEFAULT_GEAR_TABLE)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn test_size_to_mask() {
        // Test powers of 2
        assert_eq!(GearCDCChunker::size_to_mask(8), 0b111); // 3 bits for size 8 (2^3)
        assert_eq!(GearCDCChunker::size_to_mask(16), 0b1111); // 4 bits for size 16 (2^4)
        assert_eq!(GearCDCChunker::size_to_mask(8192), 0x1FFF); // 13 bits for size 8192 (2^13)
        assert_eq!(GearCDCChunker::size_to_mask(16384), 0x3FFF); // 14 bits for size 16384 (2^14)

        // Test non-powers of 2 (should round up to next power)
        assert_eq!(GearCDCChunker::size_to_mask(7), 0b111); // round up to 8 => 3 bits
        assert_eq!(GearCDCChunker::size_to_mask(9), 0b1111); // round up to 16 => 4 bits
        assert_eq!(GearCDCChunker::size_to_mask(8000), 0x1FFF); // round up to 8192 => 13 bits

        // Test edge cases
        assert_eq!(GearCDCChunker::size_to_mask(1), 0b1); // special-cased => 1 bit
        assert_eq!(GearCDCChunker::size_to_mask(2), 0b1); // log2(2) => 1 bit
    }

    #[test]
    #[should_panic(expected = "size must be between 1 and 2^63")]
    fn test_size_to_mask_zero() {
        GearCDCChunker::size_to_mask(0);
    }

    #[test]
    #[should_panic(expected = "size must be between 1 and 2^63")]
    fn test_size_to_mask_too_large() {
        GearCDCChunker::size_to_mask((1 << 63) + 1);
    }

    #[test]
    fn test_gear_hasher() {
        // Create a simple gear table for testing
        let mut gear_table = [0u64; 256];
        for i in 0..256 {
            gear_table[i] = i as u64;
        }

        let mut hasher = GearHasher::new(Arc::new(gear_table));

        // Test initial state
        assert_eq!(hasher.fingerprint(), 0);

        // Test single byte
        hasher.roll(1);
        // hash = (0 << 1) ^ 1 ^ (0 >> 53) = 1
        assert_eq!(hasher.fingerprint(), 1);

        // Test multiple bytes
        hasher.roll(2);
        // hash = (1 << 1) ^ 2 ^ (1 >> 53) = 2 ^ 2 ^ 0 = 0
        assert_eq!(hasher.fingerprint(), 0);

        hasher.roll(3);
        // hash = (0 << 1) ^ 3 ^ (0 >> 53) = 3
        assert_eq!(hasher.fingerprint(), 3);

        // Test boundary check
        assert!(hasher.boundary_check(0)); // Always true for mask 0
        assert!(!hasher.boundary_check(0x2)); // 3 & 0010 != 0
        assert!(hasher.boundary_check(0x4)); // 3 & 0100 == 0
    }

    #[test]
    fn test_gear_hasher_wrapping() {
        let gear_table = [1u64; 256]; // All 1s for simplicity
        let mut hasher = GearHasher::new(Arc::new(gear_table));

        // Roll enough times to cause wrapping
        for _ in 0..100 {
            hasher.roll(0);
        }

        // The hash should still be valid (not panicked)
        let _ = hasher.fingerprint();
    }

    #[tokio::test]
    async fn test_gearcdc_basic_chunking() -> anyhow::Result<()> {
        // Create repeatable data that should trigger chunk boundaries
        let data = b"abcdefghijklmnopqrstuvwxyz".repeat(100);

        // Use a simple gear table where each byte maps to itself
        // This makes boundary detection more predictable for testing
        let mut gear_table = [0u64; 256];
        for i in 0..256 {
            gear_table[i] = i as u64;
        }

        let chunker = GearCDCChunker::new(16, gear_table); // Small size for testing
        let mut chunk_stream = chunker.chunk(&data[..]).await?;
        let mut chunks = Vec::new();

        while let Some(chunk) = chunk_stream.next().await {
            chunks.push(chunk?);
        }

        // Basic assertions
        assert!(!chunks.is_empty(), "Should produce at least one chunk");
        assert_eq!(
            chunks.iter().map(|c| c.len()).sum::<usize>(),
            data.len(),
            "Total chunked data should equal input size"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_gearcdc_empty_input() -> anyhow::Result<()> {
        let data = b"";
        let chunker = GearCDCChunker::default();
        let mut chunk_stream = chunker.chunk(&data[..]).await?;

        assert!(
            chunk_stream.next().await.is_none(),
            "Empty input should produce no chunks"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_gearcdc_single_byte() -> anyhow::Result<()> {
        let data = b"a";
        let chunker = GearCDCChunker::default();
        let mut chunk_stream = chunker.chunk(&data[..]).await?;

        let chunk = chunk_stream.next().await.unwrap()?;
        assert_eq!(
            chunk.as_ref(),
            b"a",
            "Single byte should be returned as one chunk"
        );
        assert!(
            chunk_stream.next().await.is_none(),
            "Should only produce one chunk"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_gearcdc_chunk_distribution() -> anyhow::Result<()> {
        use rand::{rngs::StdRng, Rng, SeedableRng};

        // Test both random and repeating data to verify our high-bit feedback mechanism
        let test_cases = vec![
            ("random", {
                let mut rng = StdRng::seed_from_u64(12345);
                (0..100_000).map(|_| rng.random()).collect::<Vec<u8>>()
            }),
            ("repeating", {
                (0..100_000).map(|i| (i % 251) as u8).collect::<Vec<u8>>()
            }),
        ];

        for (data_type, data) in test_cases {
            let chunker = GearCDCChunker::new(1024, DEFAULT_GEAR_TABLE);
            let mut chunk_stream = chunker.chunk(&data[..]).await?;
            let mut chunk_sizes = Vec::new();

            while let Some(chunk) = chunk_stream.next().await {
                chunk_sizes.push(chunk?.len());
            }

            // Verify chunk size distribution
            assert!(
                !chunk_sizes.is_empty(),
                "Should produce chunks for {} data",
                data_type
            );

            // Calculate average chunk size
            let avg_size: f64 = chunk_sizes.iter().sum::<usize>() as f64 / chunk_sizes.len() as f64;

            // Most chunks should be "near" the target size
            // Allow for some variance since it's content-defined
            let target = 1024.0;
            assert!(
                (avg_size - target).abs() < target * 0.5,
                "Average chunk size for {} data ({}) should be roughly near target size ({})",
                data_type,
                avg_size,
                target
            );

            // Print distribution statistics for debugging
            println!("\n{} data statistics:", data_type);
            println!("Number of chunks: {}", chunk_sizes.len());
            println!("Average chunk size: {:.2}", avg_size);
            println!("Min chunk size: {}", chunk_sizes.iter().min().unwrap());
            println!("Max chunk size: {}", chunk_sizes.iter().max().unwrap());
        }

        Ok(())
    }
}

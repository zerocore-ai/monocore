use async_stream::try_stream;
use bytes::Bytes;
use futures::stream::BoxStream;
use std::pin::pin;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{Chunker, StoreError, StoreResult};

use super::{
    constants::DEFAULT_MAX_CHUNK_SIZE, DEFAULT_DESIRED_CHUNK_SIZE, DEFAULT_GEAR_TABLE,
    DEFAULT_MIN_CHUNK_SIZE,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A rolling hash implementation used by [`FastCDC`] to identify chunk boundaries.
pub struct FastHasher {
    /// The gear table maps each possible byte value to a pseudo-random 64-bit number.
    gear_table: [u64; 256],

    /// The current hash value, updated as new bytes are processed.
    hash: u64,
}

/// A chunker that splits data into variable-size chunks using the FastCDC algorithm.
pub struct FastCDC {
    /// The gear table.
    gear_table: [u64; 256],

    /// The desired chunk size.
    desired_chunk_size: u64,

    /// The minimum size of each chunk.
    min_chunk_size: u64,

    /// The maximum size of each chunk.
    max_chunk_size: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl FastHasher {
    /// Creates a new `FastHasher` with the given gear table.
    pub fn new(gear_table: [u64; 256]) -> Self {
        Self {
            gear_table,
            hash: 0,
        }
    }

    /// Updates the rolling hash with a new byte.
    #[inline]
    pub fn roll(&mut self, byte: u8) {
        self.hash =
            (self.hash << 1).wrapping_add(self.gear_table[byte as usize]) ^ (self.hash >> 53);
    }

    /// Returns the current hash value
    pub fn fingerprint(&self) -> u64 {
        self.hash
    }

    /// Checks if the current hash indicates a chunk boundary
    #[inline]
    pub fn boundary_check(&self, mask: u64) -> bool {
        (self.hash & mask) == 0
    }
}

impl FastCDC {
    /// Creates a new `FastCDC` with the given parameters.
    ///
    /// # Panics
    /// Panics if the chunk size parameters don't satisfy:
    /// 0 < min_chunk_size ≤ desired_chunk_size ≤ max_chunk_size ≤ 2^48
    pub fn new(
        desired_chunk_size: u64,
        min_chunk_size: u64,
        max_chunk_size: u64,
        gear_table: [u64; 256],
    ) -> Self {
        assert!(
            min_chunk_size > 0
                && min_chunk_size <= desired_chunk_size
                && desired_chunk_size <= max_chunk_size
                && max_chunk_size <= (1 << 48),
            "chunk sizes must satisfy: 0 < min ({}) ≤ desired ({}) ≤ max ({}) ≤ 2^48",
            min_chunk_size,
            desired_chunk_size,
            max_chunk_size
        );

        Self {
            gear_table,
            desired_chunk_size,
            min_chunk_size,
            max_chunk_size,
        }
    }

    /// Converts a desired chunk size to a bit mask for FastCDC.
    ///
    /// This function creates a mask with bits evenly distributed across the most significant
    /// 48 bits, leaving the lower 16 bits as zero. The number of bits set to 1 is determined
    /// by the log2 of the chunk size.
    ///
    /// For example, for a 4KiB (2^12) desired chunk size, we need 12 bits distributed across
    /// the top 48 bits of a 64-bit word:
    ///
    /// ```text
    /// Most significant 48 bits for pattern matching           Unused 16 bits
    /// ┌──────────────────────────────────────────────────────┬───────────────────┐
    /// │1000.0100.0010.0001.0000.1000.0100.0010.0001.0000.1000│0000.0000.0000.0000│
    /// └──────────────────────────────────────────────────────┴───────────────────┘
    /// ```
    ///
    /// The bits are evenly spaced to reduce the likelihood of finding chunk boundaries too
    /// frequently, which helps maintain chunk sizes closer to the desired size.
    fn size_to_mask(size: u64) -> u64 {
        const MAX_SIZE: u64 = 1 << 48;
        assert!(
            size > 0 && size <= MAX_SIZE,
            "size must be between 1 and 2^48"
        );

        // Calculate number of effective bits needed (log2 of size)
        let bits = if size == 1 {
            1
        } else {
            size.next_power_of_two().trailing_zeros() as u64
        };

        // We'll distribute these bits over the most significant 48 bits
        let spacing = 48 / bits;
        let mut mask = 0u64;

        // Place the bits evenly throughout the most significant 48 bits
        for i in 0..bits {
            // Start from bit position 63 (MSB) and work down
            // but only use the top 48 bits (positions 63 down to 16)
            mask |= 1u64 << (63 - (i * spacing));
        }

        mask
    }

    /// Derives the small and large masks from the normal (desired) mask.
    ///
    /// Instead of manipulating the mask bits directly, we derive the masks by adjusting
    /// the chunk size before applying size_to_mask:
    /// - mask_s: uses a larger chunk size (desired * 4) to get more bits
    /// - mask_l: uses a smaller chunk size (desired / 4) to get fewer bits
    ///
    /// This maintains the even distribution of bits across the mask while adjusting
    /// the number of bits to control cut point probability.
    fn derive_masks(desired_chunk_size: u64) -> (u64, u64) {
        // For mask_s: multiply chunk size by 4 (2^2) to get more bits
        let mask_s = Self::size_to_mask(desired_chunk_size << 2);

        // For mask_l: divide chunk size by 4 (2^2) to get fewer bits
        let mask_l = Self::size_to_mask(desired_chunk_size >> 2);

        (mask_s, mask_l)
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for FastHasher {
    fn default() -> Self {
        Self::new(DEFAULT_GEAR_TABLE)
    }
}

impl Default for FastCDC {
    fn default() -> Self {
        Self::new(
            DEFAULT_DESIRED_CHUNK_SIZE,
            DEFAULT_MIN_CHUNK_SIZE,
            DEFAULT_MAX_CHUNK_SIZE,
            DEFAULT_GEAR_TABLE,
        )
    }
}

impl Chunker for FastCDC {
    async fn chunk<'a>(
        &self,
        reader: impl AsyncRead + Send + 'a,
    ) -> StoreResult<BoxStream<'a, StoreResult<Bytes>>> {
        let mask_d = FastCDC::size_to_mask(self.desired_chunk_size);
        let (mask_s, mask_l) = FastCDC::derive_masks(self.desired_chunk_size);
        let gear_table = self.gear_table;
        let min_size = self.min_chunk_size;
        let max_size = self.max_chunk_size;
        let desired_size = self.desired_chunk_size;

        let s = try_stream! {
            let mut reader = pin!(reader);
            let mut current_chunk = Vec::new();
            let mut hasher = FastHasher::new(gear_table);
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

                    let chunk_len = current_chunk.len();

                    // Force a cut if we've reached max size
                    if chunk_len >= max_size as usize {
                        yield Bytes::from(current_chunk);
                        current_chunk = Vec::new();
                        continue;
                    }

                    // Only look for cut points if we've reached minimum size
                    if chunk_len >= min_size as usize {
                        // Select appropriate mask based on current chunk size
                        let mask = if chunk_len < desired_size as usize {
                            mask_l  // Use large mask (fewer bits) to decrease cut probability
                        } else if chunk_len > desired_size as usize {
                            mask_s  // Use small mask (more bits) to increase cut probability
                        } else {
                            mask_d  // Use normal mask at desired size
                        };

                        if hasher.boundary_check(mask) && !current_chunk.is_empty() {
                            yield Bytes::from(current_chunk);
                            current_chunk = Vec::new();
                        }
                    }
                }
            }
        };

        Ok(Box::pin(s))
    }

    fn chunk_max_size(&self) -> Option<u64> {
        Some(self.max_chunk_size)
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
    fn test_fastcdc_size_to_mask() {
        // Test 4KiB (2^12) case
        let mask_4k = FastCDC::size_to_mask(4096);
        println!("mask_4k: {:064b}", mask_4k);
        assert_eq!(
            mask_4k,
            0b1000_1000_1000_1000_1000_1000_1000_1000_1000_1000_1000_1000_0000_0000_0000_0000
        );

        // Verify no bits are set in the lower 16 bits
        assert_eq!(mask_4k & 0xFFFF, 0);

        // Test 8KiB (2^13) case
        let mask_8k = FastCDC::size_to_mask(8192);
        println!("mask_8k: {:064b}", mask_8k);
        assert_eq!(
            mask_8k,
            0b1001_0010_0100_1001_0010_0100_1001_0010_0100_1000_0000_0000_0000_0000_0000_0000
        );
        assert_eq!(mask_8k & 0xFFFF, 0);

        // Test edge cases
        let mask_1 = FastCDC::size_to_mask(1);
        assert_eq!(mask_1, 1u64 << 63); // Only MSB set
        assert_eq!(mask_1 & 0xFFFF, 0); // Lower 16 bits are zero

        let mask_2 = FastCDC::size_to_mask(2);
        assert_eq!(
            mask_2,
            0b1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000
        );
        assert_eq!(mask_2 & 0xFFFF, 0);
    }

    #[test]
    fn test_fastcdc_valid_chunk_sizes() {
        // Test valid chunk size combinations
        FastCDC::new(
            8192,  // desired
            4096,  // min
            16384, // max
            DEFAULT_GEAR_TABLE,
        );

        // Test edge case where min = desired = max
        FastCDC::new(
            8192, // all sizes equal
            8192,
            8192,
            DEFAULT_GEAR_TABLE,
        );

        // Test with large (but valid) sizes
        FastCDC::new(
            1 << 30, // ~1GB desired
            1 << 20, // ~1MB min
            1 << 48, // max possible size
            DEFAULT_GEAR_TABLE,
        );
    }

    #[test]
    #[should_panic(
        expected = "chunk sizes must satisfy: 0 < min (0) ≤ desired (8192) ≤ max (16384) ≤ 2^48"
    )]
    fn test_fastcdc_invalid_chunk_sizes() {
        FastCDC::new(
            8192,  // desired
            0,     // min - invalid!
            16384, // max
            DEFAULT_GEAR_TABLE,
        );
    }

    #[test]
    #[should_panic(
        expected = "chunk sizes must satisfy: 0 < min (16384) ≤ desired (8192) ≤ max (32768) ≤ 2^48"
    )]
    fn test_fastcdc_min_greater_than_desired() {
        FastCDC::new(
            8192,  // desired
            16384, // min - invalid!
            32768, // max
            DEFAULT_GEAR_TABLE,
        );
    }

    #[test]
    #[should_panic(
        expected = "chunk sizes must satisfy: 0 < min (8192) ≤ desired (16384) ≤ max (8192) ≤ 2^48"
    )]
    fn test_fastcdc_desired_greater_than_max() {
        FastCDC::new(
            16384, // desired
            8192,  // min
            8192,  // max - invalid!
            DEFAULT_GEAR_TABLE,
        );
    }

    #[test]
    #[should_panic(
        expected = "chunk sizes must satisfy: 0 < min (4096) ≤ desired (8192) ≤ max (281474976710657) ≤ 2^48"
    )]
    fn test_fastcdc_max_too_large() {
        FastCDC::new(
            8192,
            4096,
            (1u64 << 48) + 1, // max - invalid!
            DEFAULT_GEAR_TABLE,
        );
    }

    #[test]
    #[should_panic(expected = "size must be between 1 and 2^48")]
    fn test_fastcdc_size_to_mask_zero() {
        FastCDC::size_to_mask(0);
    }

    #[test]
    #[should_panic(expected = "size must be between 1 and 2^48")]
    fn test_fastcdc_size_to_mask_too_large() {
        FastCDC::size_to_mask((1 << 48) + 1);
    }

    #[test]
    fn test_fastcdc_derive_masks() {
        let desired_size = 4096; // 4KiB
        let mask_d = FastCDC::size_to_mask(desired_size);
        let (mask_s, mask_l) = FastCDC::derive_masks(desired_size);

        // Count bits in each mask
        let count_bits = |x: u64| x.count_ones();
        let bits_d = count_bits(mask_d);
        let bits_s = count_bits(FastCDC::size_to_mask(desired_size << 2)); // 16KiB
        let bits_l = count_bits(FastCDC::size_to_mask(desired_size >> 2)); // 1KiB

        // Print masks for visual inspection
        println!(
            "mask_d ({:2} bits, {:5}KiB): {:064b}",
            bits_d,
            desired_size >> 10,
            mask_d
        );
        println!(
            "mask_s ({:2} bits, {:5}KiB): {:064b}",
            bits_s,
            desired_size << 2 >> 10,
            mask_s
        );
        println!(
            "mask_l ({:2} bits, {:5}KiB): {:064b}",
            bits_l,
            desired_size >> 2 >> 10,
            mask_l
        );

        // Verify masks are derived from adjusted chunk sizes
        assert_eq!(
            mask_s,
            FastCDC::size_to_mask(desired_size << 2),
            "mask_s should be derived from size {} (desired * 4)",
            desired_size << 2
        );
        assert_eq!(
            mask_l,
            FastCDC::size_to_mask(desired_size >> 2),
            "mask_l should be derived from size {} (desired / 4)",
            desired_size >> 2
        );

        // Test with other sizes to ensure the pattern holds
        for size in [8192, 16384, 32768] {
            let (mask_s, mask_l) = FastCDC::derive_masks(size);
            assert_eq!(
                mask_s,
                FastCDC::size_to_mask(size << 2),
                "size {}: mask_s should be derived from size {}",
                size,
                size << 2
            );
            assert_eq!(
                mask_l,
                FastCDC::size_to_mask(size >> 2),
                "size {}: mask_l should be derived from size {}",
                size,
                size >> 2
            );
        }
    }

    #[tokio::test]
    async fn test_fastcdc_basic_chunking() -> anyhow::Result<()> {
        // Create repeatable data that should trigger chunk boundaries
        let data = b"abcdefghijklmnopqrstuvwxyz".repeat(100);

        // Use a simple gear table where each byte maps to itself
        // This makes boundary detection more predictable for testing
        let mut gear_table = [0u64; 256];
        for i in 0..256 {
            gear_table[i] = i as u64;
        }

        let chunker = FastCDC::new(16, 8, 32, gear_table); // Small size for testing
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
    async fn test_fastcdc_empty_input() -> anyhow::Result<()> {
        let data = b"";
        let chunker = FastCDC::default();
        let mut chunk_stream = chunker.chunk(&data[..]).await?;

        assert!(
            chunk_stream.next().await.is_none(),
            "Empty input should produce no chunks"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_fastcdc_single_byte() -> anyhow::Result<()> {
        let data = b"a";
        let chunker = FastCDC::default();
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
    async fn test_fastcdc_chunk_distribution() -> anyhow::Result<()> {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        // Test both random and repeating data to verify our normalization works
        let test_cases = vec![
            ("random", {
                let mut rng = StdRng::seed_from_u64(12345);
                (0..100_000).map(|_| rng.gen()).collect::<Vec<u8>>()
            }),
            ("repeating", {
                (0..100_000).map(|i| (i % 251) as u8).collect::<Vec<u8>>()
            }),
        ];

        for (data_type, data) in test_cases {
            println!("data_type: {:?}", data_type);
            let chunker = FastCDC::new(1024, 512, 2048, DEFAULT_GEAR_TABLE);
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

            // Verify min/max constraints
            assert!(
                chunk_sizes[..chunk_sizes.len() - 1]
                    .iter()
                    .all(|&size| size >= 512),
                "All chunks except the last should be >= min_size"
            );
            assert!(
                chunk_sizes.iter().all(|&size| size <= 2048),
                "All chunks should be <= max_size"
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

use bytes::Bytes;
use futures::stream::BoxStream;
use tokio::io::AsyncRead;

use crate::{Chunker, StoreResult};

use super::{
    constants::DEFAULT_MAX_CHUNK_SIZE, DEFAULT_DESIRED_CHUNK_SIZE, DEFAULT_GEAR_TABLE,
    DEFAULT_MIN_CHUNK_SIZE,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

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

impl FastCDC {
    /// Creates a new `FastCDC` with the given `min_size` and `max_size`.
    pub fn new(
        desired_chunk_size: u64,
        min_chunk_size: u64,
        max_chunk_size: u64,
        gear_table: [u64; 256],
    ) -> Self {
        Self {
            gear_table,
            desired_chunk_size,
            min_chunk_size,
            max_chunk_size,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Chunker for FastCDC {
    async fn chunk<'a>(
        &self,
        _reader: impl AsyncRead + Send + 'a,
    ) -> StoreResult<BoxStream<'a, StoreResult<Bytes>>> {
        let _ = _reader;
        todo!() // TODO: To be implemented
    }

    fn chunk_max_size(&self) -> Option<u64> {
        Some(self.max_chunk_size)
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

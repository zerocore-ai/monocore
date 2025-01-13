use bytes::Bytes;
use futures::stream::BoxStream;
use tokio::io::AsyncRead;

use crate::{Chunker, StoreResult};

use super::DEFAULT_CHUNK_MAX_SIZE;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A chunker that splits data into variable-size chunks using the QuickCDC algorithm.
pub struct QuickCdcChunker {
    /// The size of each chunk.
    chunk_size: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl QuickCdcChunker {
    /// Creates a new `QuickCdcChunker` with the given `chunk_size`.
    pub fn new(chunk_size: u64) -> Self {
        Self { chunk_size }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Chunker for QuickCdcChunker {
    async fn chunk<'a>(
        &self,
        _reader: impl AsyncRead + Send + 'a,
    ) -> StoreResult<BoxStream<'a, StoreResult<Bytes>>> {
        let _ = _reader;
        todo!() // TODO: To be implemented
    }

    fn chunk_max_size(&self) -> Option<u64> {
        Some(self.chunk_size)
    }
}

impl Default for QuickCdcChunker {
    fn default() -> Self {
        Self::new(DEFAULT_CHUNK_MAX_SIZE)
    }
}

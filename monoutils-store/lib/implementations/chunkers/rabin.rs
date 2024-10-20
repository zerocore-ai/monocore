use bytes::Bytes;
use futures::stream::BoxStream;
use tokio::io::AsyncRead;

use crate::{Chunker, StoreResult};

use super::DEFAULT_CHUNK_MAX_SIZE;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A chunker that splits data into variable-size chunks using the Rabin fingerprinting algorithm.
///
/// The `RabinChunker` leverages the Rabin fingerprinting technique to produce chunks of data with
/// variable sizes. This algorithm is particularly effective for identifying duplicate content within
/// files, as well as across different files, by creating consistent chunk boundaries. The resulting
/// chunks are then processed and stored in an IPLD form.
pub struct RabinChunker {
    /// The size of each chunk.
    chunk_size: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl RabinChunker {
    /// Creates a new `RabinChunker` with the given `chunk_size`.
    pub fn new(chunk_size: u64) -> Self {
        Self { chunk_size }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Chunker for RabinChunker {
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

impl Default for RabinChunker {
    fn default() -> Self {
        Self::new(DEFAULT_CHUNK_MAX_SIZE)
    }
}

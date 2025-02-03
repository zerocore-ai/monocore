use std::pin::pin;

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{Chunker, StoreError, StoreResult, DEFAULT_MAX_CHUNK_SIZE};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// `FixedSizeChunker` splits data into fixed-size chunks, regardless of the content, in a simple
/// and deterministic way.
///
/// ```text
/// Input Data:
/// ┌────────────────────────────────────────────────────┐
/// │ Lorem ipsum dolor sit amet, consectetur adipiscing │
/// └────────────────────────────────────────────────────┘
///
/// FixedSizeChunker (chunk_size = 10):
/// ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
/// │Lorem ipsu│ │m dolor si│ │t amet, co│ │nsectetur │ │adipiscing│
/// └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘
///    Chunk 1     Chunk 2      Chunk 3      Chunk 4      Chunk 5
///  (10 bytes)   (10 bytes)   (10 bytes)   (10 bytes)   (10 bytes)
/// ```
#[derive(Clone, Debug)]
pub struct FixedSizeChunker {
    /// The size of each chunk.
    chunk_size: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl FixedSizeChunker {
    /// Creates a new `FixedSizeChunker` with the given `chunk_size`.
    pub fn new(chunk_size: u64) -> Self {
        Self { chunk_size }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl Chunker for FixedSizeChunker {
    async fn chunk(
        &self,
        reader: impl AsyncRead + Send + Sync + 'life0,
    ) -> StoreResult<BoxStream<'_, StoreResult<Bytes>>> {
        let chunk_size = self.chunk_size;
        tracing::trace!("chunking with chunk size: {}", chunk_size);

        let s = try_stream! {
            let reader = pin!(reader);
            let mut chunk_reader = reader.take(chunk_size); // Derives a reader for reading the first chunk.

            loop {
                let mut chunk = vec![];
                let n = chunk_reader.read_to_end(&mut chunk).await.map_err(StoreError::custom)?;

                if n == 0 {
                    break;
                }

                tracing::trace!("yielding chunk of size: {}", chunk.len());
                yield Bytes::from(chunk);

                chunk_reader = chunk_reader.into_inner().take(chunk_size); // Derives a reader for reading the next chunk.
            }
        };

        Ok(Box::pin(s))
    }


    async fn chunk_max_size(&self) -> StoreResult<Option<u64>> {
        Ok(Some(self.chunk_size))
    }
}

impl Default for FixedSizeChunker {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CHUNK_SIZE)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn test_fixed_size_chunker() -> anyhow::Result<()> {
        let data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
        let chunker = FixedSizeChunker::new(10);

        let mut chunk_stream = chunker.chunk(&data[..]).await?;
        let mut chunks = vec![];

        while let Some(chunk) = chunk_stream.next().await {
            chunks.push(chunk?);
        }

        assert_eq!(chunks.len(), 6);
        assert_eq!(chunks[0].to_vec(), b"Lorem ipsu");
        assert_eq!(chunks[1].to_vec(), b"m dolor si");
        assert_eq!(chunks[2].to_vec(), b"t amet, co");
        assert_eq!(chunks[3].to_vec(), b"nsectetur ");
        assert_eq!(chunks[4].to_vec(), b"adipiscing");
        assert_eq!(chunks[5].to_vec(), b" elit.");

        Ok(())
    }
}

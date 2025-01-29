use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use tokio::io::AsyncRead;

use super::StoreResult;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A chunker that splits incoming bytes into chunks and returns those chunks as a stream.
///
/// This can be used by stores chunkers.
#[async_trait]
pub trait Chunker {
    /// Chunks the given reader and returns a stream of bytes.
    async fn chunk(
        &self,
        reader: impl AsyncRead + Send + Sync + 'life0,
    ) -> StoreResult<BoxStream<'_, StoreResult<Bytes>>>;

    /// Returns the allowed maximum chunk size. If there is no limit, `None` is returned.
    async fn chunk_max_size(&self) -> StoreResult<Option<u64>>;
}

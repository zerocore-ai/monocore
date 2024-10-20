use bytes::Bytes;
use futures::{stream::BoxStream, Future};
use tokio::io::AsyncRead;

use super::StoreResult;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A chunker that splits incoming bytes into chunks and returns those chunks as a stream.
///
/// This can be used by stores chunkers.
pub trait Chunker {
    /// Chunks the given reader and returns a stream of bytes.
    fn chunk<'a>(
        &self,
        reader: impl AsyncRead + Send + 'a,
    ) -> impl Future<Output = StoreResult<BoxStream<'a, StoreResult<Bytes>>>> + Send;

    /// Returns the allowed maximum chunk size. If there is no limit, `None` is returned.
    fn chunk_max_size(&self) -> Option<u64>;
}

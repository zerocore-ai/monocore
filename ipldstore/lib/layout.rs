use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use ipld_core::cid::Cid;
use monoutils::SeekableReader;
use tokio::io::AsyncRead;

use super::{IpldStore, StoreResult};

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A layout strategy for organizing a stream of chunks into a graph of blocks.
#[async_trait]
pub trait Layout {
    /// Organizes a stream of chunks into a graph of blocks storing them as either raw blocks or
    /// IPLD node blocks.
    ///
    /// Method returns a stream of `Cid`s of the blocks that were created and the last `Cid` is
    /// always the root of the graph.
    async fn organize<'a>(
        &'a self,
        stream: BoxStream<'a, StoreResult<Bytes>>,
        store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<BoxStream<'a, StoreResult<Cid>>>;

    /// Retrieves the underlying byte chunks associated with a given `Cid`.
    ///
    /// This traverses the graph of blocks to reconstruct the original byte stream.
    async fn retrieve(
        &self,
        cid: &Cid,
        store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>>;

    /// Returns the size of the underlying byte chunks associated with a given `Cid`.
    async fn get_size(&self, cid: &Cid, store: impl IpldStore + Send + Sync) -> StoreResult<u64>;
}

/// A trait that extends the `Layout` trait to allow for seeking.
#[async_trait]
pub trait LayoutSeekable: Layout {
    /// Retrieves the underlying byte chunks associated with a given `Cid` as a seekable reader.
    async fn retrieve_seekable(
        &self,
        cid: &Cid,
        store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send>>>;
}

use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use ipld_core::cid::Cid;
use tokio::io::AsyncRead;

use crate::{IpldStore, Layout, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A layout that organizes data into a balanced DAG.
#[derive(Clone, Debug, PartialEq)]
pub struct BalancedDagLayout {
    /// The maximum number of children each node can have.
    degree: usize,
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl Layout for BalancedDagLayout {
    async fn organize<'a>(
        &'a self,
        _stream: BoxStream<'a, StoreResult<Bytes>>,
        _store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<BoxStream<'a, StoreResult<Cid>>> {
        todo!() // TODO: To be implemented
    }

    async fn retrieve(
        &self,
        _cid: &Cid,
        _store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        todo!() // TODO: To be implemented
    }

    async fn get_size(&self, _cid: &Cid, _store: impl IpldStore + Send + Sync) -> StoreResult<u64> {
        todo!() // TODO: To be implemented
    }
}

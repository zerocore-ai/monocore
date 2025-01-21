use std::pin::Pin;

use bytes::Bytes;
use futures::stream::BoxStream;
use ipld_core::cid::Cid;
use tokio::io::AsyncRead;

use crate::{IpldStore, Layout, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A layout that organizes data into a trickle DAG.
#[derive(Clone, Debug, PartialEq)]
pub struct TrickleDagLayout {}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Layout for TrickleDagLayout {
    async fn organize<'a>(
        &self,
        _stream: BoxStream<'a, StoreResult<Bytes>>,
        _store: impl IpldStore + Send + Sync + 'a,
    ) -> StoreResult<BoxStream<'a, StoreResult<Cid>>> {
        todo!() // TODO: To be implemented
    }

    async fn retrieve<'a>(
        &self,
        _cid: &Cid,
        _store: impl IpldStore + Send + Sync + 'a,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send + Sync + 'a>>> {
        todo!() // TODO: To be implemented
    }

    async fn get_size(&self, _cid: &Cid, _store: impl IpldStore + Send + Sync) -> StoreResult<u64> {
        todo!() // TODO: To be implemented
    }
}

use std::{collections::HashSet, pin::Pin};

use bytes::Bytes;
use libipld::Cid;
use serde::Serialize;
use tokio::io::AsyncRead;

use crate::{Codec, IpldReferences, IpldStore, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A placeholder store that does nothing. It is meant to be used as a placeholder for a store that
/// is not yet implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceholderStore;

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl IpldStore for PlaceholderStore {
    async fn put_node<T>(&self, _: &T) -> StoreResult<Cid>
    where
        T: Serialize + IpldReferences,
    {
        unimplemented!("placeholder")
    }

    async fn put_bytes<'a>(&'a self, _: impl AsyncRead + Send + 'a) -> StoreResult<Cid> {
        unimplemented!("placeholder")
    }

    async fn put_raw_block(&self, _: impl Into<Bytes>) -> StoreResult<Cid> {
        unimplemented!("placeholder")
    }

    async fn get_node<D>(&self, _: &Cid) -> StoreResult<D>
    where
        D: serde::de::DeserializeOwned,
    {
        unimplemented!("placeholder")
    }

    async fn get_bytes<'a>(
        &'a self,
        _: &'a Cid,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send + Sync + 'a>>> {
        unimplemented!("placeholder")
    }

    async fn get_raw_block(&self, _: &Cid) -> StoreResult<Bytes> {
        unimplemented!("placeholder")
    }

    async fn has(&self, _: &Cid) -> bool {
        unimplemented!("placeholder")
    }

    fn get_supported_codecs(&self) -> HashSet<Codec> {
        unimplemented!("placeholder")
    }

    fn get_node_block_max_size(&self) -> Option<u64> {
        unimplemented!("placeholder")
    }

    fn get_raw_block_max_size(&self) -> Option<u64> {
        unimplemented!("placeholder")
    }
}

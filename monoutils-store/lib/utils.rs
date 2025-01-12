use libipld::{
    multihash::{Code, MultihashDigest},
    Cid,
};

use super::Codec;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Hashes data with [Blake3-256][blake] and returns a new [`Cid`] to it.
///
/// [blake]: https://en.wikipedia.org/wiki/BLAKE_(hash_function)
pub(crate) fn make_cid(codec: Codec, data: &[u8]) -> Cid {
    let digest = Code::Blake3_256.digest(data);
    Cid::new_v1(codec.into(), digest)
}

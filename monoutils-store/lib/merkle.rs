use libipld::Cid;
use serde::{Deserialize, Serialize};

use super::IpldReferences;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A `MerkleNode` is a simple data structure for constructing a Directed Acyclic Graph (DAG) out of
/// multiple leaf data.
///
/// It is a non-leaf data structure containing references (CIDs) to its direct dependencies which
/// can be either leaf or non-leaf data structures.
///
/// This data structure is usually used internally by `IpldStore`s to store chunked data in a way that
/// preserves the original order of the data. See [`MemoryStore`](crate::MemoryStore) for an example of
/// how this is used.
///
/// ## Important
///
/// The serialized form of this data structure is typically expected to fit in a single node block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MerkleNode {
    /// The size in bytes of the data this node represents.
    pub size: usize,

    // /// The parent of the merkle node.
    // pub parent: Option<Cid>,
    /// The direct children of this node, represented as a vector of tuples.
    /// Each tuple contains:
    /// - `Cid`: The content identifier (CID) of the child node.
    /// - `usize`: The size in bytes of the data referenced by the CID.
    pub children: Vec<(Cid, usize)>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MerkleNode {
    /// Create a new `MerkleNode` with the given size and dependencies.
    pub fn new(children: impl IntoIterator<Item = (Cid, usize)>) -> Self {
        let mut size = 0;
        let mut deps = Vec::new();
        for (cid, byte_size) in children {
            size += byte_size;
            deps.push((cid, byte_size));
        }

        Self {
            size,
            children: deps,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl IpldReferences for MerkleNode {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(self.children.iter().map(|(cid, _)| cid))
    }
}

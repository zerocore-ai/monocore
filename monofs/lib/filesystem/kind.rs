use serde::{Deserialize, Serialize};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The kind of an entity in the file system.
///
/// This corresponds to `descriptor-type` in the WASI. `monofs` does not support all the types that WASI
/// supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    /// The entity is a regular file.
    File,

    /// The entity is a directory.
    Dir,

    /// The entity is a symbolic link.
    SoftLink,
}

use monoutils_store::IpldStore;

use crate::FsResult;

use super::Dir;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Directory operations.
impl<S> Dir<S>
where
    S: IpldStore,
{
    /// Copies a directory from a source directory.
    pub fn copy_from(&self, _src: &Dir<S>) -> FsResult<()> {
        unimplemented!()
    }
}

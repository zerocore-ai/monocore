use futures::Future;
use monoutils_store::IpldStore;

use crate::FsResult;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for types that can be resolved to a target.
pub trait Resolvable<'a, S>
where
    S: IpldStore,
{
    /// The target type that the resolvable type can be resolved to.
    type Target: 'a;

    /// Resolves to a target type
    fn resolve(&'a self, store: S) -> impl Future<Output = FsResult<&'a Self::Target>>;

    /// Resolves to a mutable target type
    fn resolve_mut(&'a mut self, store: S) -> impl Future<Output = FsResult<&'a mut Self::Target>>;
}

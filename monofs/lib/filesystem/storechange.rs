use monoutils_store::IpldStore;

use crate::FsResult;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for types that can be changed to a different store.
pub trait StoreChange<S>
where
    S: IpldStore,
{
    /// The type of the entity.
    type Entity;

    /// Change the store used to persist the entity.
    fn change_store(&self, store: S) -> FsResult<Self::Entity>
    where
        Self: Sized;
}

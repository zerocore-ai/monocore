use monoutils_store::IpldStore;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for types that can be changed to a different store.
pub trait StoreSwitchable {
    /// The type of the entity.
    type WithStore<U: IpldStore>;

    /// Change the store used to persist the entity.
    fn change_store<U: IpldStore>(self, new_store: U) -> Self::WithStore<U>;
}

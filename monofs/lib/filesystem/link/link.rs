use async_once_cell::OnceCell;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between an identifier and some lazily loaded value.
pub struct Link<I, V> {
    /// The identifier of the link, e.g. a URI or CID.
    pub(crate) identifier: I,

    /// The cached value associated with the identifier.
    pub(crate) cached: Cached<V>,
}

/// A type alias for `OnceCell` holding a lazily initialized value.
pub type Cached<V> = OnceCell<V>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<I, V> Link<I, V> {
    /// Gets the cached value.
    pub fn get_cached(&self) -> Option<&V> {
        self.cached.get()
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<I, V> PartialEq for Link<I, V>
where
    I: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl<I, V> Clone for Link<I, V>
where
    I: Clone,
{
    fn clone(&self) -> Self {
        Link {
            identifier: self.identifier.clone(),
            cached: OnceCell::new(),
        }
    }
}

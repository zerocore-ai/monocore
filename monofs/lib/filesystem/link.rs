mod cidlink;
mod pathlink;

use async_once_cell::OnceCell;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A type alias for `OnceCell` holding a lazily initialized value.
pub type Cached<V> = OnceCell<V>;

/// A link representing an association between an identifier and some lazily loaded value or
/// just the value itself.
pub enum Link<I, V> {
    /// A link that is encoded and needs to be resolved.
    Encoded {
        /// The identifier of the link, e.g. a URI or CID.
        identifier: I,

        /// The cached value associated with the identifier.
        cached: Cached<V>,
    },

    /// A link that is decoded and can be used directly.
    Decoded(V),
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<I, V> Clone for Link<I, V>
where
    I: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Encoded { identifier, .. } => Self::Encoded {
                identifier: identifier.clone(),
                cached: Cached::new(),
            },
            Self::Decoded(value) => Self::Decoded(value.clone()),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use cidlink::*;
pub use pathlink::*;

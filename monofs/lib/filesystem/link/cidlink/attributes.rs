use crate::filesystem::{CidLink, ExtendedAttributes};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Cid`] and a lazily loaded [`ExtendedAttributes`].
pub type AttributeCidLink<S> = CidLink<ExtendedAttributes<S>>;

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

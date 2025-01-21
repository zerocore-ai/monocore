use std::iter;

use bytes::Bytes;
use ipld_core::cid::Cid;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for types that can hold [CID][cid] references to some data.
///
/// [cid]: https://docs.ipfs.tech/concepts/content-addressing/
pub trait IpldReferences {
    /// Returns all the direct CID references the type has to other data.
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a>;
}

//--------------------------------------------------------------------------------------------------
// Macros
//--------------------------------------------------------------------------------------------------

macro_rules! impl_ipld_references {
    (($($name:ident),+)) => {
        impl<$($name),+> IpldReferences for ($($name,)+)
        where
            $($name: IpldReferences,)*
        {
            fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                Box::new(
                    Vec::new().into_iter()
                    $(.chain($name.get_references()))+
                )
            }
        }
    };
    ($type:ty) => {
        impl IpldReferences for $type {
            fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
                Box::new(std::iter::empty())
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

// Nothing
impl_ipld_references!(());

// Scalars
impl_ipld_references!(bool);
impl_ipld_references!(u8);
impl_ipld_references!(u16);
impl_ipld_references!(u32);
impl_ipld_references!(u64);
impl_ipld_references!(u128);
impl_ipld_references!(usize);
impl_ipld_references!(i8);
impl_ipld_references!(i16);
impl_ipld_references!(i32);
impl_ipld_references!(i64);
impl_ipld_references!(i128);
impl_ipld_references!(isize);
impl_ipld_references!(f32);
impl_ipld_references!(f64);

// Containers
impl_ipld_references!(Vec<u8>);
impl_ipld_references!(&[u8]);
impl_ipld_references!(Bytes);
impl_ipld_references!(String);
impl_ipld_references!(&str);

// Tuples
impl_ipld_references!((A, B));
impl_ipld_references!((A, B, C));
impl_ipld_references!((A, B, C, D));
impl_ipld_references!((A, B, C, D, E));
impl_ipld_references!((A, B, C, D, E, F));
impl_ipld_references!((A, B, C, D, E, F, G));
impl_ipld_references!((A, B, C, D, E, F, G, H));

impl<T> IpldReferences for Option<T>
where
    T: IpldReferences,
{
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        match self {
            Some(value) => Box::new(value.get_references()),
            None => Box::new(iter::empty()),
        }
    }
}

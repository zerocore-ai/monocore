use futures::future::BoxFuture;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// An async version of the `PartialEq` trait.
pub trait AsyncPartialEq<Rhs: ?Sized + Send + Sync = Self>
where
    Self: Send + Sync,
{
    /// This method tests for `self` and `other` values to be equal, and returns a future that
    /// resolves to `true` if they are equal.
    fn eq<'a>(&'a self, other: &'a Rhs) -> BoxFuture<'a, bool>;

    /// This method tests for `!=` and returns a future that resolves to `true` if the values are not equal.
    fn ne<'a>(&'a self, other: &'a Rhs) -> BoxFuture<'a, bool>
    where
        Self: 'a,
    {
        Box::pin(async move { !self.eq(other).await })
    }
}

/// An async version of the `Eq` trait.
pub trait AsyncEq: AsyncPartialEq {}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<T> AsyncPartialEq<T> for T
where
    T: PartialEq + Send + Sync,
{
    fn eq<'a>(&'a self, other: &'a T) -> BoxFuture<'a, bool> {
        Box::pin(async move { self == other })
    }
}

//--------------------------------------------------------------------------------------------------
// Macros
//--------------------------------------------------------------------------------------------------

/// ...
#[macro_export]
macro_rules! async_assert {
    ($lhs:expr, $rhs:expr) => {
        assert!($lhs.await)
    };
}

/// ...
#[macro_export]
macro_rules! async_assert_eq {
    ($lhs:expr, $rhs:expr) => {
        assert!($lhs.eq($rhs).await)
    };
}

/// ...
#[macro_export]
macro_rules! async_assert_ne {
    ($lhs:expr, $rhs:expr) => {
        assert!($lhs.ne($rhs).await)
    };
}

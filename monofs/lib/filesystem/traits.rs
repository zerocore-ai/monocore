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
    fn eq(&self, other: &Rhs) -> BoxFuture<bool>;

    /// This method tests for `!=` and returns a future that resolves to `true` if the values are not equal.
    fn ne<'a>(&'a self, other: &'a Rhs) -> BoxFuture<bool>
    where
        Self: 'a,
    {
        Box::pin(async move { !self.eq(other).await })
    }
}

/// An async version of the `Eq` trait.
pub trait AsyncEq: AsyncPartialEq {}

use std::{
    fmt::{self, Debug, Display},
    str::FromStr,
};

use async_once_cell::OnceCell;
use monoutils_store::{ipld::cid::Cid, IpldStore, Storable};

use crate::{Cached, FsError, FsResult, Link};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A link representing an association between [`Cid`] and some lazily loaded value.
pub type CidLink<T> = Link<Cid, T>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<T> CidLink<T> {
    /// Gets the CID of the link.
    pub fn get_cid(&self) -> &Cid {
        &self.identifier
    }

    /// Creates a new [`CidLink`] from a value.
    pub async fn from_value<S>(value: T) -> FsResult<Self>
    where
        T: Storable<S>,
        S: IpldStore,
    {
        let cid = value.store().await?;
        Ok(Self {
            identifier: cid,
            cached: Cached::new_with(value),
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<T> From<Cid> for CidLink<T> {
    fn from(cid: Cid) -> Self {
        Self {
            identifier: cid,
            cached: OnceCell::new(),
        }
    }
}

impl<T> FromStr for CidLink<T> {
    type Err = FsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cid = Cid::from_str(s)?;
        Ok(Self::from(cid))
    }
}

impl<T> TryFrom<String> for CidLink<T> {
    type Error = FsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cid = Cid::try_from(value)?;
        Ok(Self::from(cid))
    }
}

impl<T> Display for CidLink<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.identifier)
    }
}

impl<T> From<CidLink<T>> for Cid {
    fn from(link: CidLink<T>) -> Self {
        link.identifier
    }
}

impl<T> Debug for CidLink<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CidLink")
            .field("identifier", &self.identifier.to_string())
            .field("cached", &self.cached.get())
            .finish()
    }
}

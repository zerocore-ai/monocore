use monoutils_store::IpldStore;

use crate::FsResult;

use super::Dir;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Directory operations.
impl<S> Dir<S>
where
    S: IpldStore,
{
    /// ...
    pub fn find(&self) -> FsResult<()> {
        unimplemented!()
    }

    /// ...
    pub fn finsert(&self) -> FsResult<()> {
        unimplemented!()
    }

    /// ...
    pub fn copy(&self) -> FsResult<()> {
        unimplemented!()
    }

    /// ...
    pub fn r#move(&self) -> FsResult<()> {
        unimplemented!()
    }

    /// ...
    #[inline]
    pub fn mv(&self) -> FsResult<()> {
        unimplemented!()
    }

    /// ...
    pub fn remove(&self) -> FsResult<()> {
        unimplemented!()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::MemoryStore;

    use super::*;

    #[tokio::test]
    async fn test_find() -> FsResult<()> {
        let store = MemoryStore::default();
        let _dir = Dir::new(store.clone());

        Ok(())
    }
}

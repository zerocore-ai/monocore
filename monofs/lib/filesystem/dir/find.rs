use std::fmt::Debug;

use monoutils_store::IpldStore;
use typed_path::{Utf8UnixComponent, Utf8UnixPath};

use crate::filesystem::{entity::Entity, FsError, FsResult};

use super::{Dir, Utf8UnixPathSegment};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Result type for `find_dir*` functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindResult<T> {
    /// The directory was found.
    Found {
        /// The directory containing the entity.
        dir: T,
    },

    /// The directory was not found.
    NotFound {
        /// The last found directory in the path.
        dir: T,

        /// The depth of the path to the entity.
        depth: usize,
    },

    /// Intermediate path is not a directory.
    NotADir {
        /// The depth of the path to the entity.
        depth: usize,
    },
}

/// Result type for `find_dir` function.
pub type FindResultDir<'a, S> = FindResult<&'a Dir<S>>;

/// Result type for `find_dir_mut` function.
pub type FindResultDirMut<'a, S> = FindResult<&'a mut Dir<S>>;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Looks for a directory at the specified path.
///
/// This function navigates through the directory structure starting from the given directory,
/// following the path specified by `path`. It attempts to resolve each component of the path
/// until it either finds the target directory, encounters an error, or determines that the path
/// is not found or invalid.
///
/// ## Examples
///
/// ```
/// use monofs::filesystem::{Dir, find_dir};
/// use monoutils_store::MemoryStore;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let store = MemoryStore::default();
/// let root_dir = Dir::new(store);
/// let result = find_dir(&root_dir, "some/path/to/entity").await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Note
///
/// The function does not support the following path components:
/// - `.`
/// - `..`
/// - `/`
///
/// If any of these components are present in the path, the function will return an error.
pub async fn find_dir<S>(mut dir: &Dir<S>, path: impl AsRef<str>) -> FsResult<FindResultDir<S>>
where
    S: IpldStore + Send + Sync,
{
    let path = Utf8UnixPath::new(path.as_ref());

    // Convert path components to Utf8UnixPathSegment and collect them
    let components = path
        .components()
        .map(|ref c| match c {
            Utf8UnixComponent::RootDir => Err(FsError::InvalidSearchPath(path.to_string())),
            Utf8UnixComponent::CurDir => Err(FsError::InvalidSearchPath(path.to_string())),
            Utf8UnixComponent::ParentDir => Err(FsError::InvalidSearchPath(path.to_string())),
            _ => Ok(Utf8UnixPathSegment::try_from(c)?),
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Process intermediate components (if any)
    for (depth, segment) in components.iter().enumerate() {
        match dir.get_entity(segment).await? {
            Some(Entity::Dir(d)) => {
                dir = d;
            }
            Some(Entity::SoftLink(_)) => {
                // SoftLinks are not supported yet, so we return an error
                return Err(FsError::SoftLinkNotSupportedYet(components.clone()));
            }
            Some(_) => {
                // If we encounter a non-directory entity in the middle of the path,
                // we return NotADir result
                return Ok(FindResult::NotADir { depth });
            }
            None => {
                // If an intermediate component doesn't exist,
                // we return NotFound result
                return Ok(FindResult::NotFound { dir, depth });
            }
        }
    }

    Ok(FindResult::Found { dir })
}

/// Looks for a directory at the specified path. This is a mutable version of `find_dir`.
///
/// This function navigates through the directory structure starting from the given directory,
/// following the path specified by `path`. It attempts to resolve each component of the path
/// until it either finds the target directory, encounters an error, or determines that the path
/// is not found or invalid.
///
/// ## Examples
///
/// ```
/// use monofs::filesystem::{Dir, find_dir_mut};
/// use monoutils_store::MemoryStore;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let store = MemoryStore::default();
/// let mut root_dir = Dir::new(store);
/// let result = find_dir_mut(&mut root_dir, "some/path/to/entity").await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Note
///
/// The function does not support the following path components:
/// - `.`
/// - `..`
/// - `/`
///
/// If any of these components are present in the path, the function will return an error.
pub async fn find_dir_mut<S>(
    mut dir: &mut Dir<S>,
    path: impl AsRef<str>,
) -> FsResult<FindResultDirMut<S>>
where
    S: IpldStore + Send + Sync,
{
    let path = Utf8UnixPath::new(path.as_ref());

    // Convert path components to Utf8UnixPathSegment and collect them
    let components = path
        .components()
        .map(|ref c| match c {
            Utf8UnixComponent::RootDir => Err(FsError::InvalidSearchPath(path.to_string())),
            Utf8UnixComponent::CurDir => Err(FsError::InvalidSearchPath(path.to_string())),
            Utf8UnixComponent::ParentDir => Err(FsError::InvalidSearchPath(path.to_string())),
            _ => Ok(Utf8UnixPathSegment::try_from(c)?),
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Process intermediate components (if any)
    for (depth, segment) in components.iter().enumerate() {
        match dir.get_entity(segment).await? {
            Some(Entity::Dir(_)) => {
                // A hack to get a mutable reference to the directory
                dir = dir.get_dir_mut(segment).await?.unwrap();
            }
            Some(Entity::SoftLink(_)) => {
                // SoftLinks are not supported yet, so we return an error
                return Err(FsError::SoftLinkNotSupportedYet(components.clone()));
            }
            Some(_) => {
                // If we encounter a non-directory entity in the middle of the path,
                // we return NotADir result
                return Ok(FindResult::NotADir { depth });
            }
            None => {
                // If an intermediate component doesn't exist,
                // we return NotFound result
                return Ok(FindResult::NotFound { dir, depth });
            }
        }
    }

    Ok(FindResult::Found { dir })
}

/// Retrieves an existing entity or creates a new one at the specified path.
///
/// This function checks the existence of an entity at the given path. If the entity
/// exists, it returns the entity. If the entity does not exist, it creates a new
/// directory hierarchy and returns the new entity.
///
/// ## Examples
///
/// ```
/// use monofs::filesystem::{Dir, find_or_create_dir};
/// use monoutils_store::MemoryStore;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let store = MemoryStore::default();
/// let mut root_dir = Dir::new(store);
/// let new_dir = find_or_create_dir(&mut root_dir, "new/nested/directory").await?;
/// assert!(new_dir.is_empty());
/// # Ok(())
/// # }
/// ```
pub async fn find_or_create_dir<S>(dir: &mut Dir<S>, path: impl AsRef<str>) -> FsResult<&mut Dir<S>>
where
    S: IpldStore + Send + Sync,
{
    let path = Utf8UnixPath::new(path.as_ref());

    match find_dir_mut(dir, path).await {
        Ok(FindResult::Found { dir }) => Ok(dir),
        Ok(FindResult::NotFound { mut dir, depth }) => {
            for component in path.components().skip(depth) {
                let new_dir = Dir::new(dir.get_store().clone());
                let segment = Utf8UnixPathSegment::try_from(&component)?;

                dir.put_dir(segment.clone(), new_dir)?;
                dir = dir.get_dir_mut(&segment).await?.unwrap();
            }

            Ok(dir)
        }
        Ok(FindResult::NotADir { depth }) => {
            let components = path
                .components()
                .take(depth + 1)
                .map(|c| c.to_string())
                .collect::<Vec<_>>();

            Err(FsError::NotADirectory(components.join("/")))
        }
        Err(e) => Err(e),
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::MemoryStore;

    use crate::filesystem::File;

    use super::*;

    mod fixtures {
        use monoutils_store::Storable;

        use super::*;

        pub(super) async fn setup_test_filesystem() -> anyhow::Result<Dir<MemoryStore>> {
            let store = MemoryStore::default();
            let mut root = Dir::new(store.clone());

            let mut subdir1 = Dir::new(store.clone());
            let mut subdir2 = Dir::new(store.clone());

            let file1 = File::new(store.clone());
            let file2 = File::new(store.clone());

            let file1_cid = file1.store().await?;
            subdir1.put_entry("file1.txt", file1_cid.into())?;

            let file2_cid = file2.store().await?;
            subdir2.put_entry("file2.txt", file2_cid.into())?;

            let subdir2_cid = subdir2.store().await?;
            subdir1.put_entry("subdir2", subdir2_cid.into())?;

            let subdir1_cid = subdir1.store().await?;
            root.put_entry("subdir1", subdir1_cid.into())?;

            Ok(root)
        }
    }

    #[tokio::test]
    async fn test_find_dir() -> anyhow::Result<()> {
        let root = fixtures::setup_test_filesystem().await?;

        // Test finding existing directories
        let result = find_dir(&root, "subdir1").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let result = find_dir(&root, "subdir1/subdir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test finding non-existent directories
        let result = find_dir(&root, "nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 0, .. }));

        let result = find_dir(&root, "subdir1/nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 1, .. }));

        // Test finding a path that contains a file
        let result = find_dir(&root, "subdir1/file1.txt/invalid").await?;
        assert!(matches!(result, FindResult::NotADir { depth: 1 }));

        // Test invalid paths
        let result = find_dir(&root, "/invalid/path").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir(&root, "invalid/../path").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir(&root, "./invalid/path").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_find_dir_mut() -> anyhow::Result<()> {
        let mut root = fixtures::setup_test_filesystem().await?;

        // Test finding existing directories
        let result = find_dir_mut(&mut root, "subdir1").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let result = find_dir_mut(&mut root, "subdir1/subdir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test finding non-existent directories
        let result = find_dir_mut(&mut root, "nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 0, .. }));

        let result = find_dir_mut(&mut root, "subdir1/nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 1, .. }));

        // Test finding a path that contains a file
        let result = find_dir_mut(&mut root, "subdir1/file1.txt/invalid").await?;
        assert!(matches!(result, FindResult::NotADir { depth: 1 }));

        Ok(())
    }

    #[tokio::test]
    async fn test_find_or_create_dir() -> anyhow::Result<()> {
        let mut root = fixtures::setup_test_filesystem().await?;

        // Test creating a new directory
        let new_dir = find_or_create_dir(&mut root, "new_dir").await?;
        assert!(new_dir.is_empty());

        // Verify the new directory exists
        let result = find_dir(&root, "new_dir").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test creating a nested structure
        let nested_dir = find_or_create_dir(&mut root, "parent/child/grandchild").await?;
        assert!(nested_dir.is_empty());

        // Verify the nested structure exists
        let result = find_dir(&root, "parent/child/grandchild").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test getting an existing directory
        let existing_dir = find_or_create_dir(&mut root, "subdir1").await?;
        assert!(!existing_dir.is_empty());

        // Test creating a directory where a file already exists
        let result = find_or_create_dir(&mut root, "subdir1/file1.txt").await;
        assert!(matches!(result, Err(FsError::NotADirectory(_))));

        Ok(())
    }
}

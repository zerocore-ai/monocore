use std::fmt::Debug;

use monoutils_store::IpldStore;
use typed_path::{Utf8UnixComponent, Utf8UnixPath};

use crate::{filesystem::entity::Entity, FsError, FsResult};

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
pub(crate) async fn find_dir<S>(
    mut dir: &Dir<S>,
    path: impl AsRef<str>,
) -> FsResult<FindResultDir<S>>
where
    S: IpldStore + Send + Sync,
{
    // Normalize the path first - this will handle . and .. components and validate the path
    let normalized_path =
        monoutils::normalize_path(path.as_ref(), monoutils::SupportedPathType::Relative)
            .map_err(|_| FsError::InvalidSearchPath(path.as_ref().to_string()))?;

    let components = Utf8UnixPath::new(&normalized_path)
        .components()
        .filter_map(|c| match c {
            Utf8UnixComponent::Normal(s) => Some(Utf8UnixPathSegment::try_from(s)),
            _ => None, // Skip any . or .. since they were handled by normalize_path
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Process intermediate components (if any)
    for (depth, segment) in components.iter().enumerate() {
        match dir.get_entity(segment).await? {
            Some(Entity::Dir(d)) => {
                dir = d;
            }
            Some(Entity::SymCidLink(_)) => {
                // SymCidLinks are not supported yet, so we return an error
                return Err(FsError::SymCidLinkNotSupportedYet(components));
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
pub(crate) async fn find_dir_mut<S>(
    mut dir: &mut Dir<S>,
    path: impl AsRef<str>,
) -> FsResult<FindResultDirMut<S>>
where
    S: IpldStore + Send + Sync,
{
    // Normalize the path first - this will handle . and .. components and validate the path
    let normalized_path =
        monoutils::normalize_path(path.as_ref(), monoutils::SupportedPathType::Relative)
            .map_err(|_| FsError::InvalidSearchPath(path.as_ref().to_string()))?;

    let components = Utf8UnixPath::new(&normalized_path)
        .components()
        .filter_map(|c| match c {
            Utf8UnixComponent::Normal(s) => Some(Utf8UnixPathSegment::try_from(s)),
            _ => None, // Skip any . or .. since they were handled by normalize_path
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Process intermediate components (if any)
    for (depth, segment) in components.iter().enumerate() {
        match dir.get_entity(segment).await? {
            Some(Entity::Dir(_)) => {
                // A hack to get a mutable reference to the directory
                dir = dir.get_dir_mut(segment).await?.unwrap();
            }
            Some(Entity::SymCidLink(_)) => {
                // SymCidLinks are not supported yet, so we return an error
                return Err(FsError::SymCidLinkNotSupportedYet(components));
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
pub(crate) async fn find_or_create_dir<S>(
    dir: &mut Dir<S>,
    path: impl AsRef<str>,
) -> FsResult<&mut Dir<S>>
where
    S: IpldStore + Send + Sync,
{
    match find_dir_mut(dir, path.as_ref()).await {
        Ok(FindResult::Found { dir }) => Ok(dir),
        Ok(FindResult::NotFound { mut dir, depth }) => {
            let normalized_path =
                monoutils::normalize_path(path.as_ref(), monoutils::SupportedPathType::Relative)
                    .map_err(|_| FsError::InvalidSearchPath(path.as_ref().to_string()))?;

            let components = Utf8UnixPath::new(&normalized_path)
                .components()
                .skip(depth)
                .filter_map(|c| match c {
                    Utf8UnixComponent::Normal(s) => Some(Utf8UnixPathSegment::try_from(s)),
                    _ => None, // Skip any . or .. since they were handled by normalize_path
                })
                .collect::<Result<Vec<_>, _>>()?;

            for segment in components {
                let new_dir = Dir::new(dir.get_store().clone());
                dir.put_adapted_dir(segment.clone(), new_dir).await?;
                dir = dir.get_dir_mut(&segment).await?.unwrap();
            }

            Ok(dir)
        }
        Ok(FindResult::NotADir { depth }) => {
            let normalized_path =
                monoutils::normalize_path(path.as_ref(), monoutils::SupportedPathType::Relative)
                    .map_err(|_| FsError::InvalidSearchPath(path.as_ref().to_string()))?;

            let components = Utf8UnixPath::new(&normalized_path)
                .components()
                .take(depth + 1)
                .filter_map(|c| match c {
                    Utf8UnixComponent::Normal(s) => Some(s.to_string()),
                    _ => None,
                })
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
            subdir1
                .put_adapted_entry("file1.txt", file1_cid.into())
                .await?;

            let file2_cid = file2.store().await?;
            subdir2
                .put_adapted_entry("file2.txt", file2_cid.into())
                .await?;

            let subdir2_cid = subdir2.store().await?;
            subdir1
                .put_adapted_entry("subdir2", subdir2_cid.into())
                .await?;

            let subdir1_cid = subdir1.store().await?;
            root.put_adapted_entry("subdir1", subdir1_cid.into())
                .await?;

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

        // Test with . and .. components that resolve within bounds
        let result = find_dir(&root, "subdir1/./subdir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let result = find_dir(&root, "subdir1/subdir2/../subdir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let result = find_dir(&root, "./subdir1").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test finding non-existent directories
        let result = find_dir(&root, "nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 0, .. }));

        let result = find_dir(&root, "subdir1/nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 1, .. }));

        // Test finding a path that contains a file
        let result = find_dir(&root, "subdir1/file1.txt/invalid").await?;
        assert!(matches!(result, FindResult::NotADir { depth: 1 }));

        // Test path escape attempts - these should fail
        let result = find_dir(&root, "..").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir(&root, "../escaped").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir(&root, "subdir1/../../escaped").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        // Test invalid paths
        let result = find_dir(&root, "/invalid/path").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir(&root, "").await;
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

        // Test with . and .. components that resolve within bounds
        let result = find_dir_mut(&mut root, "subdir1/./subdir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let result = find_dir_mut(&mut root, "subdir1/subdir2/../subdir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let result = find_dir_mut(&mut root, "./subdir1").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test finding non-existent directories
        let result = find_dir_mut(&mut root, "nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 0, .. }));

        let result = find_dir_mut(&mut root, "subdir1/nonexistent").await?;
        assert!(matches!(result, FindResult::NotFound { depth: 1, .. }));

        // Test finding a path that contains a file
        let result = find_dir_mut(&mut root, "subdir1/file1.txt/invalid").await?;
        assert!(matches!(result, FindResult::NotADir { depth: 1 }));

        // Test path escape attempts - these should fail
        let result = find_dir_mut(&mut root, "..").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir_mut(&mut root, "../escaped").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir_mut(&mut root, "subdir1/../../escaped").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        // Test invalid paths
        let result = find_dir_mut(&mut root, "/invalid/path").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_dir_mut(&mut root, "").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

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

        // Test with . and .. components that resolve within bounds
        let dir1 = find_or_create_dir(&mut root, "test/./dir1").await?;
        assert!(dir1.is_empty());
        let result = find_dir(&root, "test/dir1").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        let dir2 = find_or_create_dir(&mut root, "test/temp/../dir2").await?;
        assert!(dir2.is_empty());
        let result = find_dir(&root, "test/dir2").await?;
        assert!(matches!(result, FindResult::Found { .. }));

        // Test getting an existing directory
        let existing_dir = find_or_create_dir(&mut root, "subdir1").await?;
        assert!(!existing_dir.is_empty());

        // Test creating a directory where a file already exists
        let result = find_or_create_dir(&mut root, "subdir1/file1.txt").await;
        assert!(matches!(result, Err(FsError::NotADirectory(_))));

        // Test path escape attempts - these should fail
        let result = find_or_create_dir(&mut root, "..").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_or_create_dir(&mut root, "../escaped").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_or_create_dir(&mut root, "test/../../escaped").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        // Test invalid paths
        let result = find_or_create_dir(&mut root, "/invalid/path").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        let result = find_or_create_dir(&mut root, "").await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        Ok(())
    }
}

use std::fmt::{self, Debug};

use monoutils_store::IpldStore;
use typed_path::{Utf8UnixComponent, Utf8UnixPath};

use crate::{file::File, filesystem::entity::Entity, FsError, FsResult, PathDirs};

use super::{Dir, Utf8UnixPathSegment};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) enum FindResult<S>
where
    S: IpldStore,
{
    /// The entity was found.
    Found {
        /// The entity found.
        entity: Entity<S>,

        /// The name of the entity in its parent directory entries. `None` if the handle has
        /// no parent directory.
        name: Option<Utf8UnixPathSegment>,

        /// The directories along the path to the entity.
        pathdirs: PathDirs<S>,
    },

    /// The entity was not found.
    Incomplete {
        /// The directories along the path to the entity.
        pathdirs: PathDirs<S>,

        /// The depth of the path to the entity.
        depth: usize,
    },

    /// Intermediate path is not a directory.
    NotADir {
        /// The directories along the path to the entity.
        pathdirs: PathDirs<S>,

        /// The depth of the path to the entity.
        depth: usize,
    },
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Finds an existing entity at the specified path within a directory structure.
///
/// This function navigates through the directory structure starting from the given directory,
/// following the path specified by `path`. It attempts to resolve each component of the path
/// until it either finds the target entity, encounters an error, or determines that the path
/// is incomplete or invalid.
///
/// ## Examples
///
/// ```
/// use monocore::filesystem::dir::{Dir, find_entity};
/// use monoutils_store::MemoryStore;
/// use typed_path::Utf8UnixPath;
///
/// # async fn example() -> anyhow::Result<()> {
/// let store = MemoryStore::default();
/// let root_dir = Dir::new(store);
/// let path = Utf8UnixPath::new("some/path/to/entity");
/// let result = find_entity(&root_dir, path).await?;
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
#[allow(unused)] // TODO: Remove this once the function is used
pub(crate) async fn find_entity<'a, S>(
    mut dir: &Dir<S>,
    path: &Utf8UnixPath,
) -> FsResult<FindResult<S>>
where
    S: IpldStore + Send + Sync,
{
    if path.components().any(|c| {
        matches!(
            c,
            Utf8UnixComponent::RootDir | Utf8UnixComponent::CurDir | Utf8UnixComponent::ParentDir
        )
    }) {
        return Err(FsError::InvalidSearchPath(path.to_string()));
    }

    // Convert path components to Utf8UnixPathSegment and collect them
    let components = path
        .components()
        .map(|ref c| Utf8UnixPathSegment::try_from(c))
        .collect::<Result<Vec<_>, _>>()?;

    // Split the components into intermediate and final parts
    let (intermediate_components, final_component) = match components.len() {
        0 => return Err(FsError::InvalidSearchPathEmpty),
        1 => (None, &components[0]),
        _ => {
            let (intermediates, last) = components.split_at(components.len() - 1);
            (Some(intermediates), &last[0])
        }
    };

    // Process intermediate components (if any)
    let mut pathdirs = PathDirs::new();
    let mut depth = 0;
    if let Some(intermediates) = intermediate_components {
        for segment in intermediates.iter() {
            // Keep track of the path we've traversed so far
            pathdirs.push((dir.clone(), segment.clone()));

            match dir.get_entity(segment).await? {
                Some(Entity::Dir(d)) => {
                    dir = d;
                }
                Some(Entity::Symlink(_)) => {
                    // Symlinks are not supported yet, so we return an error
                    return Err(FsError::SymLinkNotSupportedYet(components.clone()));
                }
                Some(_) => {
                    // If we encounter a non-directory entity in the middle of the path,
                    // we return NotADir result
                    return Ok(FindResult::NotADir { pathdirs, depth });
                }
                None => {
                    // If an intermediate component doesn't exist,
                    // we return Incomplete result
                    return Ok(FindResult::Incomplete { pathdirs, depth });
                }
            }

            depth += 1;
        }
    }

    // Push the final component to the pathdirs
    pathdirs.push((dir.clone(), final_component.clone()));

    // Process the final component
    match dir.get_entity(final_component).await? {
        Some(entity) => Ok(FindResult::Found {
            entity: entity.clone(),
            name: Some(final_component.clone()),
            pathdirs,
        }),
        None => Ok(FindResult::Incomplete { pathdirs, depth }),
    }
}

/// Retrieves an existing entity or creates a new one at the specified path.
///
/// This function checks the existence of an entity at the given path. If the entity
/// exists, it returns the entity and its corresponding path directories. If the
/// entity does not exist, it creates a new directory hierarchy and returns the new
/// entity and its corresponding path directories.
///
/// `file` argument indicates whether to create a file (`true`) or a directory (`false`)
/// if the entity does not exist.
///
/// ## Returns
///
///  Function returns `(Entity<S>, Option<PathSegment>, PathDirs<S>)` on `Ok` result.
///  - **Entity\<S\>**: The entity found or created.
///  - **Option\<PathSegment\>**: The name of the entity in its parent directory.
///  - **PathDirs\<S\>**: The directories along the path to the entity.
#[allow(unused)] // TODO: Remove this once the function is used
pub(crate) async fn find_or_create_entity<S>(
    dir: &Dir<S>,
    path: &Utf8UnixPath,
    file: bool,
) -> FsResult<(Entity<S>, Option<Utf8UnixPathSegment>, PathDirs<S>)>
where
    S: IpldStore + Send + Sync,
{
    match find_entity(dir, path).await {
        Ok(FindResult::Found {
            entity,
            name,
            pathdirs,
        }) => Ok((entity, name, pathdirs)),
        Ok(FindResult::Incomplete {
            mut pathdirs,
            depth,
        }) => {
            let components: Vec<_> = path.components().collect();
            let total_components = components.len();

            // Set the last directory in the pathdirs as the current directory.
            let mut current_dir = pathdirs.last().unwrap().0.clone();

            // Add the remaining path components to the directory, except for the last one
            for segment in components
                .iter()
                .skip(depth)
                .take(total_components - depth - 1)
            {
                let child_dir = Dir::new(dir.get_store().clone());
                let name = Utf8UnixPathSegment::try_from(segment)?;

                current_dir.put_dir(name.clone(), child_dir.clone()).await?;
                pathdirs.push((current_dir, name));

                current_dir = child_dir;
            }

            // Create the final entity (file or directory)
            let last_segment = Utf8UnixPathSegment::try_from(components.last().unwrap())?;
            let entity = if file {
                let new_file = File::new(dir.get_store().clone());
                current_dir
                    .put_file(last_segment.clone(), new_file.clone())
                    .await?;
                Entity::File(new_file)
            } else {
                let new_dir = Dir::new(dir.get_store().clone());
                current_dir
                    .put_dir(last_segment.clone(), new_dir.clone())
                    .await?;
                Entity::Dir(new_dir)
            };

            Ok((entity, Some(last_segment), pathdirs))
        }
        Ok(FindResult::NotADir { depth, .. }) => {
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
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> Debug for FindResult<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FindResult::Found {
                entity,
                name,
                pathdirs,
            } => f
                .debug_struct("Found")
                .field("entity", entity)
                .field("name", name)
                .field("pathdirs", pathdirs)
                .finish(),
            FindResult::NotADir { pathdirs, depth } => f
                .debug_struct("NotADir")
                .field("pathdirs", pathdirs)
                .field("depth", depth)
                .finish(),
            FindResult::Incomplete { pathdirs, depth } => f
                .debug_struct("Incomplete")
                .field("pathdirs", pathdirs)
                .field("depth", depth)
                .finish(),
        }
    }
}

impl<S> PartialEq for FindResult<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                FindResult::Found {
                    entity: e1,
                    name: n1,
                    pathdirs: p1,
                },
                FindResult::Found {
                    entity: e2,
                    name: n2,
                    pathdirs: p2,
                },
            ) => e1 == e2 && n1 == n2 && p1 == p2,
            (
                FindResult::Incomplete {
                    pathdirs: p1,
                    depth: d1,
                },
                FindResult::Incomplete {
                    pathdirs: p2,
                    depth: d2,
                },
            ) => p1 == p2 && d1 == d2,
            (
                FindResult::NotADir {
                    pathdirs: p1,
                    depth: d1,
                },
                FindResult::NotADir {
                    pathdirs: p2,
                    depth: d2,
                },
            ) => p1 == p2 && d1 == d2,
            _ => false,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::{MemoryStore, Storable};
    use typed_path::Utf8UnixPathBuf;

    use crate::file::File;

    use super::*;

    mod fixtures {
        use super::*;

        pub(super) async fn setup_test_filesystem() -> anyhow::Result<Dir<MemoryStore>> {
            let store = MemoryStore::default();
            let mut root = Dir::new(store.clone());

            // Create a directory structure
            let mut subdir1 = Dir::new(store.clone());
            let mut subdir2 = Dir::new(store.clone());

            let file1 = File::new(store.clone());
            let file2 = File::new(store.clone());

            let file1_cid = file1.store().await?;
            subdir1.put("file1.txt".parse()?, file1_cid.into())?;

            let file2_cid = file2.store().await?;
            subdir2.put("file2.txt".parse()?, file2_cid.into())?;

            let subdir2_cid = subdir2.store().await?;
            subdir1.put("subdir2".parse()?, subdir2_cid.into())?;

            let subdir1_cid = subdir1.store().await?;
            root.put("subdir1".parse()?, subdir1_cid.into())?;

            Ok(root)
        }
    }

    #[tokio::test]
    async fn test_find_entity() -> anyhow::Result<()> {
        let root = fixtures::setup_test_filesystem().await?;

        let subdir1 = root
            .get_entity(&"subdir1".parse()?)
            .await?
            .cloned()
            .unwrap()
            .into_dir()
            .unwrap();

        let file1 = subdir1
            .get_entity(&"file1.txt".parse()?)
            .await?
            .cloned()
            .unwrap()
            .into_file()
            .unwrap();

        let subdir2 = subdir1
            .get_entity(&"subdir2".parse()?)
            .await?
            .cloned()
            .unwrap()
            .into_dir()
            .unwrap();

        let file2 = subdir2
            .get_entity(&"file2.txt".parse()?)
            .await?
            .cloned()
            .unwrap()
            .into_file()
            .unwrap();

        // Test cases
        let test_cases = vec![
            // Positive cases
            (
                "subdir1",
                FindResult::Found {
                    entity: Entity::Dir(subdir1.clone()),
                    name: Some("subdir1".parse()?),
                    pathdirs: {
                        let mut pd = PathDirs::new();
                        pd.push((root.clone(), "subdir1".parse()?));
                        pd
                    },
                },
            ),
            (
                "subdir1/file1.txt",
                FindResult::Found {
                    entity: Entity::File(file1.clone()),
                    name: Some("file1.txt".parse()?),
                    pathdirs: {
                        let mut pd = PathDirs::new();
                        pd.push((root.clone(), "subdir1".parse()?));
                        pd.push((subdir1.clone(), "file1.txt".parse()?));
                        pd
                    },
                },
            ),
            (
                "subdir1/subdir2/file2.txt",
                FindResult::Found {
                    entity: Entity::File(file2.clone()),
                    name: Some("file2.txt".parse()?),
                    pathdirs: {
                        let mut pd = PathDirs::new();
                        pd.push((root.clone(), "subdir1".parse()?));
                        pd.push((subdir1.clone(), "subdir2".parse()?));
                        pd.push((subdir2.clone(), "file2.txt".parse()?));
                        pd
                    },
                },
            ),
            // Negative cases
            (
                "nonexistent",
                FindResult::Incomplete {
                    pathdirs: {
                        let mut pd = PathDirs::new();
                        pd.push((root.clone(), "nonexistent".parse()?));
                        pd
                    },
                    depth: 0,
                },
            ),
            (
                "subdir1/nonexistent",
                FindResult::Incomplete {
                    pathdirs: {
                        let mut pd = PathDirs::new();
                        pd.push((root.clone(), "subdir1".parse()?));
                        pd.push((subdir1.clone(), "nonexistent".parse()?));
                        pd
                    },
                    depth: 1,
                },
            ),
            (
                "subdir1/file1.txt/invalid",
                FindResult::NotADir {
                    pathdirs: {
                        let mut pd = PathDirs::new();
                        pd.push((root.clone(), "subdir1".parse()?));
                        pd.push((subdir1.clone(), "file1.txt".parse()?));
                        pd
                    },
                    depth: 1,
                },
            ),
        ];

        for (path, expected_result) in test_cases {
            let result = find_entity(&root, &Utf8UnixPathBuf::from(path)).await?;
            assert_eq!(result, expected_result, "Failed for path: {}", path);
        }

        // Test case for invalid path with root
        let invalid_path = "/invalid/path";
        let result = find_entity(&root, &Utf8UnixPathBuf::from(invalid_path)).await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        // Test case for invalid path with ".."
        let invalid_path = "invalid/../path";
        let result = find_entity(&root, &Utf8UnixPathBuf::from(invalid_path)).await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        // Test case for invalid path with "."
        let invalid_path = "./invalid/path";
        let result = find_entity(&root, &Utf8UnixPathBuf::from(invalid_path)).await;
        assert!(matches!(result, Err(FsError::InvalidSearchPath(_))));

        Ok(())
    }
}

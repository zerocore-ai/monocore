use ipldstore::{ipld::cid::Cid, IpldStore};
use typed_path::Utf8UnixPath;

use crate::{
    filesystem::{dir::find, entity::Entity, file::File, SymCidLink, SymPathLink},
    utils::path,
    FsError, FsResult,
};

use super::{Dir, FindResult, Utf8UnixPathSegment};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Directory operations.
impl<S> Dir<S>
where
    S: IpldStore + Send + Sync,
{
    /// Finds an entity in the directory structure given a path.
    ///
    /// This method traverses the directory structure to find the entity specified by the path.
    /// It returns a reference to the found entity if it exists and is not marked as deleted.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo/bar.txt", true).await?;
    ///
    /// let entity = dir.find("foo/bar.txt").await?;
    /// assert!(matches!(entity, Some(Entity::File(_))));
    ///
    /// // After removing, find returns None
    /// dir.remove("foo/bar.txt").await?;
    /// assert!(dir.find("foo/bar.txt").await?.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find(&self, path: impl AsRef<str>) -> FsResult<Option<&Entity<S>>> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        if let Some(parent_path) = parent {
            return match find::find_dir(self, parent_path).await? {
                FindResult::Found { dir } => dir.get_entity(&file_name).await,
                _ => Ok(None),
            };
        }

        self.get_entity(&file_name).await
    }

    /// Finds an entity in the directory structure given a path, returning a mutable reference.
    ///
    /// This method is similar to `find`, but it returns a mutable reference to the found entity.
    /// It will skip entities that are marked as deleted.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo/bar.txt", true).await?;
    ///
    /// let entity = dir.find_mut("foo/bar.txt").await?;
    /// assert!(matches!(entity, Some(Entity::File(_))));
    ///
    /// // After removing, find_mut returns None
    /// dir.remove("foo/bar.txt").await?;
    /// assert!(dir.find_mut("foo/bar.txt").await?.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find_mut(&mut self, path: impl AsRef<str>) -> FsResult<Option<&mut Entity<S>>> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        if let Some(parent_path) = parent {
            return match find::find_dir_mut(self, parent_path).await? {
                FindResult::Found { dir } => dir.get_entity_mut(&file_name).await,
                _ => Ok(None),
            };
        }

        self.get_entity_mut(&file_name).await
    }

    /// Finds an entity in the directory structure or creates it if it doesn't exist.
    ///
    /// This method traverses the directory structure to find the entity specified by the path.
    /// If any part of the path doesn't exist, it will be created. This includes creating all
    /// necessary parent directories.
    ///
    /// ## Arguments
    /// * `path` - The path to find or create. Can be a single name or a nested path (e.g., "foo/bar/baz.txt")
    /// * `file` - If true, creates a file at the final path component. If false, creates a directory
    ///
    /// ## Examples
    ///
    /// Creating nested directories:
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    ///
    /// // Creates all parent directories automatically
    /// let nested_dir = dir.find_or_create("docs/guides/tutorial", false).await?;
    /// assert!(matches!(nested_dir, Entity::Dir(_)));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Creating a file in a nested path:
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let mut dir = Dir::new(MemoryStore::default());
    /// // Creates parent directories and the file
    /// let file = dir.find_or_create("projects/rust/main.rs", true).await?;
    /// assert!(matches!(file, Entity::File(_)));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Finding existing entities:
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let mut dir = Dir::new(MemoryStore::default());
    /// // Create a file first
    /// dir.find_or_create("config.json", true).await?;
    ///
    /// // Later find the same file
    /// let existing = dir.find_or_create("config.json", true).await?;
    /// assert!(matches!(existing, Entity::File(_)));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - If the path exists but is of a different type (file vs directory), the existing entity
    ///   will be returned without modification
    /// - Parent directories are always created as directories, regardless of the `file` parameter
    /// - The method is atomic - either all parts of the path are created or none are
    /// - Deleted entities are treated as non-existent and will be recreated
    pub async fn find_or_create(
        &mut self,
        path: impl AsRef<str>,
        file: bool,
    ) -> FsResult<&mut Entity<S>> {
        tracing::trace!("find_or_create: path: {:?}, file: {}", path.as_ref(), file);
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        let parent_dir = match parent {
            Some(parent_path) => find::find_or_create_dir(self, parent_path).await?,
            None => self,
        };

        if parent_dir.has_entry(&file_name)? {
            return parent_dir
                .get_entity_mut(&file_name)
                .await?
                .ok_or_else(|| FsError::PathNotFound(path.to_string()));
        }

        let new_entity = if file {
            Entity::File(File::new(parent_dir.get_store().clone()))
        } else {
            Entity::Dir(Dir::new(parent_dir.get_store().clone()))
        };

        parent_dir
            .put_adapted_entity(file_name.clone(), new_entity)
            .await?;

        parent_dir
            .get_entity_mut(&file_name)
            .await?
            .ok_or_else(|| FsError::PathNotFound(path.to_string()))
    }

    async fn create_entity(
        &mut self,
        path: impl AsRef<str>,
        entity: impl Into<Entity<S>>,
    ) -> FsResult<&mut Entity<S>> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        let parent_dir = if let Some(parent_path) = parent {
            match find::find_dir_mut(self, parent_path).await? {
                FindResult::Found { dir } => dir,
                _ => return Err(FsError::PathNotFound(parent_path.to_string())),
            }
        } else {
            self
        };

        if parent_dir.has_entry(&file_name)? {
            return Err(FsError::PathExists(path.to_string()));
        }

        parent_dir
            .put_adapted_entity(file_name.clone(), entity)
            .await?;

        parent_dir
            .get_entity_mut(&file_name)
            .await?
            .ok_or_else(|| FsError::PathNotFound(path.to_string()))
    }

    /// Creates a file at the specified path.
    #[inline]
    pub async fn create_file(&mut self, path: impl AsRef<str>) -> FsResult<&mut File<S>> {
        tracing::trace!("create_file: path: {:?}", path.as_ref());
        match self
            .create_entity(path, File::new(self.get_store().clone()))
            .await?
        {
            Entity::File(file) => Ok(file),
            _ => unreachable!(),
        }
    }

    /// Creates a directory at the specified path.
    #[inline]
    pub async fn create_dir(&mut self, path: impl AsRef<str>) -> FsResult<&mut Dir<S>> {
        tracing::trace!("create_dir: path: {:?}", path.as_ref());
        match self
            .create_entity(path, Dir::new(self.get_store().clone()))
            .await?
        {
            Entity::Dir(dir) => Ok(dir),
            _ => unreachable!(),
        }
    }

    /// Creates a symbolic path link at the specified path.
    #[inline]
    pub async fn create_sympathlink(
        &mut self,
        path: impl AsRef<str>,
        target: impl AsRef<str>,
    ) -> FsResult<&mut SymPathLink<S>> {
        tracing::trace!(
            "create_sympathlink: path: {:?}, target: {:?}",
            path.as_ref(),
            target.as_ref()
        );
        match self
            .create_entity(
                path,
                SymPathLink::with_path(self.get_store().clone(), target)?,
            )
            .await?
        {
            Entity::SymPathLink(link) => Ok(link),
            _ => unreachable!(),
        }
    }

    /// Creates a symbolic CID link at the specified path.
    #[inline]
    pub async fn create_symcidlink(
        &mut self,
        path: impl AsRef<str>,
        cid: Cid,
    ) -> FsResult<&mut SymCidLink<S>> {
        tracing::trace!(
            "create_symcidlink: path: {:?}, cid: {:?}",
            path.as_ref(),
            cid
        );
        match self
            .create_entity(path, SymCidLink::with_cid(self.get_store().clone(), cid))
            .await?
        {
            Entity::SymCidLink(link) => Ok(link),
            _ => unreachable!(),
        }
    }

    /// Lists all entries in the current directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::Dir;
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo", false).await?;
    /// dir.find_or_create("bar.txt", true).await?;
    /// dir.find_or_create("baz/qux.txt", true).await?;
    ///
    /// let entries = dir.list().collect::<Vec<_>>();
    /// assert_eq!(entries.len(), 3);
    /// assert!(entries.contains(&"foo".parse()?));
    /// assert!(entries.contains(&"bar.txt".parse()?));
    /// assert!(entries.contains(&"baz".parse()?));
    /// # Ok(())
    /// # }
    /// ```
    pub fn list(&self) -> impl Iterator<Item = Utf8UnixPathSegment> + '_ {
        tracing::trace!("list");
        self.get_entries().map(|(k, _)| k.clone())
    }

    /// Copies an entity from the source path to the target **directory**.
    ///
    /// The target path must be a directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("source/file.txt", true).await?;
    /// dir.find_or_create("target", false).await?;
    ///
    /// dir.copy("source/file.txt", "target").await?;
    ///
    /// assert!(dir.find("target/file.txt").await?.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy(&mut self, source: impl AsRef<str>, target: impl AsRef<str>) -> FsResult<()> {
        tracing::trace!(
            "copy: source: {:?}, target: {:?}",
            source.as_ref(),
            target.as_ref()
        );
        let source = Utf8UnixPath::new(source.as_ref());
        let target = Utf8UnixPath::new(target.as_ref());

        if source.has_root() || target.has_root() {
            return Err(FsError::PathHasRoot(source.to_string()));
        }

        let (source_parent, source_filename) = path::split_last(source)?;

        // Find source parent directory and entity
        let source_entity = if let Some(parent_path) = source_parent {
            let parent_dir = self
                .find(parent_path)
                .await?
                .and_then(|entity| {
                    if let Entity::Dir(dir) = entity {
                        Some(dir)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| FsError::SourceIsNotADir(parent_path.to_string()))?;

            parent_dir
                .get_entity(&source_filename)
                .await?
                .cloned()
                .ok_or_else(|| FsError::PathNotFound(source.to_string()))?
        } else {
            self.get_entity(&source_filename)
                .await?
                .cloned()
                .ok_or_else(|| FsError::PathNotFound(source.to_string()))?
        };

        // Find target directory
        let target_dir = match self.find_mut(target).await? {
            Some(Entity::Dir(dir)) => dir,
            _ => return Err(FsError::TargetIsNotADir(target.to_string())),
        };

        // Copy entity to target directory
        target_dir
            .put_adapted_entity(source_filename, source_entity)
            .await?;

        Ok(())
    }

    /// Removes an entity at the specified path by marking it as deleted.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo/bar.txt", true).await?;
    ///
    /// dir.remove("foo/bar.txt").await?;
    /// assert!(dir.find("foo/bar.txt").await?.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn remove(&mut self, path: impl AsRef<str>) -> FsResult<()> {
        tracing::trace!("remove: path: {:?}", path.as_ref());
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, filename) = path::split_last(path)?;

        let parent_dir = if let Some(parent_path) = parent {
            self.find_mut(parent_path)
                .await?
                .and_then(|entity| {
                    if let Entity::Dir(dir) = entity {
                        Some(dir)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| FsError::SourceIsNotADir(parent_path.to_string()))?
        } else {
            self
        };

        parent_dir.remove_entry(&filename)?;
        Ok(())
    }

    /// Renames (moves) an entity from one path to another.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity};
    /// use ipldstore::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    ///
    /// // Create a file
    /// dir.find_or_create("old/file.txt", true).await?;
    /// dir.find_or_create("new", false).await?;
    ///
    /// // Rename the file
    /// dir.rename("old/file.txt", "new/file.txt").await?;
    ///
    /// assert!(dir.find("old/file.txt").await?.is_none());
    /// assert!(dir.find("new/file.txt").await?.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn rename(
        &mut self,
        old_path: impl AsRef<str>,
        new_path: impl AsRef<str>,
    ) -> FsResult<()> {
        tracing::trace!(
            "rename: old_path: {:?}, new_path: {:?}",
            old_path.as_ref(),
            new_path.as_ref()
        );
        let old_path = Utf8UnixPath::new(old_path.as_ref());
        let new_path = Utf8UnixPath::new(new_path.as_ref());

        if old_path.has_root() || new_path.has_root() {
            return Err(FsError::PathHasRoot(old_path.to_string()));
        }

        // Get the parent directories and filenames for both paths
        let (old_parent, old_filename) = path::split_last(old_path)?;
        let (new_parent, new_filename) = path::split_last(new_path)?;

        // First check if target exists to fail fast
        let target_exists = if let Some(parent_path) = new_parent {
            match find::find_dir(self, parent_path).await? {
                find::FindResult::Found { dir } => dir.has_entry(&new_filename)?,
                _ => return Err(FsError::PathNotFound(new_path.to_string())),
            }
        } else {
            self.has_entry(&new_filename)?
        };

        if target_exists {
            return Err(FsError::PathExists(new_path.to_string()));
        }

        // Get the source entity without removing it
        let source_entity = if let Some(parent_path) = old_parent {
            match find::find_dir(self, parent_path).await? {
                find::FindResult::Found { dir } => dir.get_entry(&old_filename)?,
                _ => return Err(FsError::PathNotFound(old_path.to_string())),
            }
        } else {
            self.get_entry(&old_filename)?
        };

        let source_entity = source_entity
            .ok_or_else(|| FsError::PathNotFound(old_path.to_string()))?
            .clone();

        // Add the entity to the new location
        let result = if let Some(parent_path) = new_parent {
            match find::find_dir_mut(self, parent_path).await? {
                find::FindResult::Found { dir } => {
                    dir.put_adapted_entry(new_filename, source_entity.clone())
                        .await
                }
                _ => Err(FsError::PathNotFound(new_path.to_string())),
            }
        } else {
            self.put_adapted_entry(new_filename, source_entity.clone())
                .await
        };

        // Only remove from old location if adding to new location succeeded
        if result.is_ok() {
            if let Some(parent_path) = old_parent {
                match find::find_dir_mut(self, parent_path).await? {
                    find::FindResult::Found { dir } => dir.remove_entry(&old_filename)?,
                    _ => return Err(FsError::PathNotFound(old_path.to_string())),
                };
            } else {
                self.remove_entry(&old_filename)?;
            }
        }

        result
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use ipldstore::{ipld::cid::Cid, MemoryStore, Storable};
    use tokio::io::AsyncReadExt;

    use crate::filesystem::{symcidlink::SymCidLink, sympathlink::SymPathLink, Dir, Entity, File};

    use super::*;

    mod fixtures {
        use super::*;

        pub(super) async fn setup_test_filesystem() -> FsResult<Dir<MemoryStore>> {
            let store = MemoryStore::default();
            let mut root = Dir::new(store.clone());

            // Create a complex nested structure
            root.find_or_create("projects/web/index.html", true).await?;
            root.find_or_create("projects/web/styles/main.css", true)
                .await?;
            root.find_or_create("projects/app/src/main.rs", true)
                .await?;
            root.find_or_create("documents/personal/notes.txt", true)
                .await?;
            root.find_or_create("documents/work/report.pdf", true)
                .await?;

            Ok(root)
        }
    }

    #[tokio::test]
    async fn test_ops_find() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a file and a subdirectory
        dir.find_or_create("foo/bar.txt", true).await?;
        dir.find_or_create("baz", false).await?;

        // Test finding existing entities
        assert!(matches!(
            dir.find("foo/bar.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(dir.find("baz").await?, Some(Entity::Dir(_))));

        // Test finding non-existent entity
        assert!(dir.find("nonexistent").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_find_mut() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a file and a subdirectory
        dir.find_or_create("foo/bar.txt", true).await?;
        dir.find_or_create("baz", false).await?;

        // Test finding existing entities mutably
        assert!(matches!(
            dir.find_mut("foo/bar.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(dir.find_mut("baz").await?, Some(Entity::Dir(_))));

        // Test finding non-existent entity
        assert!(dir.find_mut("nonexistent").await?.is_none());

        // Test modifying a found file
        if let Some(Entity::File(file)) = dir.find_mut("foo/bar.txt").await? {
            let content = "Hello, World!".as_bytes();
            let content_cid = file.get_store().put_bytes(content).await?;
            file.set_content(Some(content_cid));
            file.store().await?;
            assert_eq!(file.get_content(), Some(&content_cid));
        } else {
            panic!("Expected to find a file");
        }

        // Verify the modification persists
        if let Some(Entity::File(file)) = dir.find("foo/bar.txt").await? {
            let content_cid = file.get_content().expect("File should have content");
            let mut content = Vec::new();
            file.get_store()
                .get_bytes(&content_cid)
                .await?
                .read_to_end(&mut content)
                .await?;
            assert_eq!(content, "Hello, World!".as_bytes());
        } else {
            panic!("Expected to find a file");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_find_or_create() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a file
        assert!(dir.find("foo/bar.txt").await?.is_none());
        let file = dir.find_or_create("foo/bar.txt", true).await?;
        assert!(matches!(file, Entity::File(_)));

        // Create a directory
        assert!(dir.find("baz").await?.is_none());
        let subdir = dir.find_or_create("baz", false).await?;
        assert!(matches!(subdir, Entity::Dir(_)));

        // Find existing entities
        let existing_file = dir.find("foo/bar.txt").await?;
        assert!(matches!(existing_file, Some(Entity::File(_))));

        let existing_dir = dir.find("baz").await?;
        assert!(matches!(existing_dir, Some(Entity::Dir(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_list() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create some entries
        dir.find_or_create("foo", false).await?;
        dir.find_or_create("bar.txt", true).await?;
        dir.find_or_create("baz/qux.txt", true).await?;

        // List entries
        let entries = dir.list().collect::<Vec<_>>();

        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&"foo".parse()?));
        assert!(entries.contains(&"bar.txt".parse()?));
        assert!(entries.contains(&"baz".parse()?));

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_copy() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a source file
        assert!(dir.find("source/file.txt").await?.is_none());
        assert!(dir.find("target").await?.is_none());
        dir.find_or_create("source/file.txt", true).await?;
        dir.find_or_create("target", false).await?;

        // Copy the file
        assert!(dir.find("target/file.txt").await?.is_none());
        dir.copy("source/file.txt", "target").await?;

        // Verify the copy
        assert!(matches!(
            dir.find("source/file.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            dir.find("target/file.txt").await?,
            Some(Entity::File(_))
        ));

        // Test copying a directory
        assert!(dir.find("source/subdir").await?.is_none());
        dir.find_or_create("source/subdir", false).await?;

        assert!(dir.find("target/subdir").await?.is_none());
        dir.copy("source/subdir", "target").await?;

        assert!(matches!(
            dir.find("source/subdir").await?,
            Some(Entity::Dir(_))
        ));
        assert!(matches!(
            dir.find("target/subdir").await?,
            Some(Entity::Dir(_))
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_remove() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Test 1: Remove a file from root directory
        dir.find_or_create("file1.txt", true).await?;
        assert!(dir.find("file1.txt").await?.is_some());
        dir.remove("file1.txt").await?;
        assert!(dir.find("file1.txt").await?.is_none());

        // Test 2: Remove a file from nested directory
        dir.find_or_create("nested/file2.txt", true).await?;
        assert!(dir.find("nested/file2.txt").await?.is_some());
        dir.remove("nested/file2.txt").await?;
        assert!(dir.find("nested/file2.txt").await?.is_none());
        // Parent directory should still exist
        assert!(matches!(dir.find("nested").await?, Some(Entity::Dir(_))));

        // Test 3: Remove an empty directory
        dir.find_or_create("empty_dir", false).await?;
        assert!(dir.find("empty_dir").await?.is_some());
        dir.remove("empty_dir").await?;
        assert!(dir.find("empty_dir").await?.is_none());

        // Test 4: Remove a directory with contents
        dir.find_or_create("dir/subdir/file3.txt", true).await?;
        dir.find_or_create("dir/file4.txt", true).await?;
        assert!(dir.find("dir").await?.is_some());
        dir.remove("dir").await?;
        assert!(dir.find("dir").await?.is_none());
        assert!(dir.find("dir/subdir/file3.txt").await?.is_none());
        assert!(dir.find("dir/file4.txt").await?.is_none());

        // Test 5: Attempt to remove non-existent path
        assert!(matches!(
            dir.remove("nonexistent.txt").await,
            Err(FsError::PathNotFound(_))
        ));

        // Test 6: Attempt to remove with root path
        assert!(matches!(
            dir.remove("/file.txt").await,
            Err(FsError::PathHasRoot(_))
        ));

        // Test 7: Remove symlinks
        let target_cid = Cid::default();
        dir.create_symcidlink("cid_link", target_cid).await?;
        dir.create_sympathlink("path_link", "target/path").await?;

        assert!(matches!(
            dir.find("cid_link").await?,
            Some(Entity::SymCidLink(_))
        ));
        assert!(matches!(
            dir.find("path_link").await?,
            Some(Entity::SymPathLink(_))
        ));

        dir.remove("cid_link").await?;
        dir.remove("path_link").await?;

        assert!(dir.find("cid_link").await?.is_none());
        assert!(dir.find("path_link").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_find_with_deleted() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create test entities
        dir.find_or_create("file1.txt", true).await?;
        dir.find_or_create("file2.txt", true).await?;
        dir.find_or_create("dir1", false).await?;

        // Mark file1.txt as deleted
        dir.remove("file1.txt").await?;

        // Test find
        assert!(dir.find("file1.txt").await?.is_none()); // Should skip deleted
        assert!(matches!(
            dir.find("file2.txt").await?.unwrap(),
            Entity::File(_)
        )); // Should find non-deleted
        assert!(matches!(dir.find("dir1").await?.unwrap(), Entity::Dir(_))); // Should find non-deleted

        // Test find_mut
        assert!(dir.find_mut("file1.txt").await?.is_none()); // Should skip deleted
        assert!(matches!(
            dir.find_mut("file2.txt").await?.unwrap(),
            Entity::File(_)
        )); // Should find non-deleted
        assert!(matches!(
            dir.find_mut("dir1").await?.unwrap(),
            Entity::Dir(_)
        )); // Should find non-deleted

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_complex_nested_hierarchy() -> anyhow::Result<()> {
        let mut root = fixtures::setup_test_filesystem().await?;

        // Verify the structure
        assert!(matches!(
            root.find("projects").await?.unwrap(),
            Entity::Dir(_)
        ));
        assert!(matches!(
            root.find("projects/web/index.html").await?.unwrap(),
            Entity::File(_)
        ));
        assert!(matches!(
            root.find("projects/app/src/main.rs").await?.unwrap(),
            Entity::File(_)
        ));
        assert!(matches!(
            root.find("documents/work/report.pdf").await?.unwrap(),
            Entity::File(_)
        ));

        // List contents of directories
        let root_contents = root.list().collect::<Vec<_>>();
        assert_eq!(root_contents.len(), 2);
        assert!(root_contents.contains(&"projects".parse()?));
        assert!(root_contents.contains(&"documents".parse()?));

        if let Some(Entity::Dir(projects_dir)) = root.find("projects").await? {
            let projects_contents = projects_dir.list().collect::<Vec<_>>();
            assert_eq!(projects_contents.len(), 2);
            assert!(projects_contents.contains(&"web".parse()?));
            assert!(projects_contents.contains(&"app".parse()?));
        } else {
            panic!("Expected to find 'projects' directory");
        }

        // Modify a file
        if let Some(Entity::File(index_file)) = root.find_mut("projects/web/index.html").await? {
            let content = "<html><body>Hello, World!</body></html>".as_bytes();
            let content_cid = index_file.get_store().put_bytes(content).await?;
            index_file.set_content(Some(content_cid));
            index_file.store().await?;
        } else {
            panic!("Expected to find 'index.html' file");
        }

        // Verify the modification
        if let Some(Entity::File(index_file)) = root.find("projects/web/index.html").await? {
            let content_cid = index_file.get_content().expect("File should have content");
            let mut content = Vec::new();
            index_file
                .get_store()
                .get_bytes(&content_cid)
                .await?
                .read_to_end(&mut content)
                .await?;
            assert_eq!(
                content,
                "<html><body>Hello, World!</body></html>".as_bytes()
            );
        } else {
            panic!("Expected to find 'index.html' file");
        }

        // Copy a file
        root.copy("documents/personal/notes.txt", "projects")
            .await?;
        assert!(matches!(
            root.find("projects/notes.txt").await?.unwrap(),
            Entity::File(_)
        ));

        // Remove report.pdf
        root.remove("documents/work/report.pdf").await?;
        assert!(root.find("documents/work/report.pdf").await?.is_none());

        // Remove notes.txt
        root.remove("documents/personal/notes.txt").await?;
        assert!(root.find("documents/personal/notes.txt").await?.is_none());

        // Remove personal directory
        root.remove("documents/personal").await?;
        assert!(root.find("documents/personal").await?.is_none());

        // Verify the final structure
        assert!(matches!(
            root.find("projects/web/index.html").await?.unwrap(),
            Entity::File(_)
        ));
        assert!(matches!(
            root.find("projects/app/src/main.rs").await?.unwrap(),
            Entity::File(_)
        ));
        assert!(matches!(
            root.find("projects/notes.txt").await?.unwrap(),
            Entity::File(_)
        ));
        assert!(root.find("documents/personal").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_rename() -> anyhow::Result<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Test 1: Basic file rename to new location
        dir.find_or_create("source/file.txt", true).await?;
        dir.find_or_create("target", false).await?;
        dir.rename("source/file.txt", "target/newfile.txt").await?;
        assert!(dir.find("source/file.txt").await?.is_none());
        assert!(matches!(
            dir.find("target/newfile.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 2: Rename within same directory (just name change)
        dir.find_or_create("samedir/original.txt", true).await?;
        dir.rename("samedir/original.txt", "samedir/renamed.txt")
            .await?;
        assert!(dir.find("samedir/original.txt").await?.is_none());
        assert!(matches!(
            dir.find("samedir/renamed.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 3: Directory rename with nested content and verify content preservation
        dir.find_or_create("olddir/subdir/file1.txt", true).await?;
        dir.find_or_create("olddir/subdir/file2.txt", true).await?;

        // Add some content to verify it's preserved
        if let Some(Entity::File(file)) = dir.find_mut("olddir/subdir/file1.txt").await? {
            let content = "test content".as_bytes();
            let content_cid = file.get_store().put_bytes(content).await?;
            file.set_content(Some(content_cid));
            file.store().await?;
        }

        dir.rename("olddir", "newdir").await?;

        // Verify structure and content preservation
        assert!(dir.find("olddir").await?.is_none());
        assert!(matches!(dir.find("newdir").await?, Some(Entity::Dir(_))));
        assert!(matches!(
            dir.find("newdir/subdir/file1.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            dir.find("newdir/subdir/file2.txt").await?,
            Some(Entity::File(_))
        ));

        // Verify content was preserved
        if let Some(Entity::File(file)) = dir.find("newdir/subdir/file1.txt").await? {
            let content_cid = file.get_content().expect("File should have content");
            let mut content = Vec::new();
            file.get_store()
                .get_bytes(&content_cid)
                .await?
                .read_to_end(&mut content)
                .await?;
            assert_eq!(content, "test content".as_bytes());
        }

        // Test 4: Rename across different directory depths
        dir.find_or_create("shallow/file.txt", true).await?;
        dir.find_or_create("deep/path/to/dir", false).await?;
        dir.rename("shallow/file.txt", "deep/path/to/dir/file.txt")
            .await?;
        assert!(dir.find("shallow/file.txt").await?.is_none());
        assert!(matches!(
            dir.find("deep/path/to/dir/file.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 5: Rename with special characters in filename
        dir.find_or_create("special/file-with-dashes.txt", true)
            .await?;
        dir.rename(
            "special/file-with-dashes.txt",
            "special/file with spaces.txt",
        )
        .await?;
        assert!(matches!(
            dir.find("special/file with spaces.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 6: Verify error cases
        // Test 6.1: Rename to existing path
        dir.find_or_create("file1.txt", true).await?;
        dir.find_or_create("file2.txt", true).await?;
        assert!(dir.rename("file1.txt", "file2.txt").await.is_err());
        assert!(dir.find("file1.txt").await?.is_some()); // Original file should still exist
        assert!(dir.find("file2.txt").await?.is_some());

        // Test 6.2: Rename non-existent source
        assert!(dir.rename("nonexistent.txt", "newfile.txt").await.is_err());

        // Test 6.3: Rename to non-existent target directory
        assert!(dir
            .rename("file1.txt", "nonexistent/newfile.txt")
            .await
            .is_err());
        assert!(dir.find("file1.txt").await?.is_some()); // Original file should still exist

        // Test 6.4: Rename with root paths
        assert!(dir.rename("/file1.txt", "newfile.txt").await.is_err());
        assert!(dir.rename("file1.txt", "/newfile.txt").await.is_err());

        // Test 7: Store state verification
        // Create a complex rename scenario and verify store state
        dir.find_or_create("state_test/source/file.txt", true)
            .await?;

        // Create target directory first
        dir.find_or_create("state_test/target", false).await?;

        if let Some(Entity::File(file)) = dir.find_mut("state_test/source/file.txt").await? {
            let content = "store state test".as_bytes();
            let content_cid = file.get_store().put_bytes(content).await?;
            file.set_content(Some(content_cid));
            file.store().await?;
        }

        dir.rename(
            "state_test/source/file.txt",
            "state_test/target/renamed.txt",
        )
        .await?;

        // Verify store state after rename
        if let Some(Entity::File(file)) = dir.find("state_test/target/renamed.txt").await? {
            let content_cid = file.get_content().expect("File should have content");
            let mut content = Vec::new();
            file.get_store()
                .get_bytes(&content_cid)
                .await?
                .read_to_end(&mut content)
                .await?;
            assert_eq!(content, "store state test".as_bytes());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_create_entity() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Test 1: Create a file in root directory
        let file = dir
            .create_entity("file.txt", File::new(store.clone()))
            .await?;
        assert!(matches!(file, Entity::File(_)));
        assert!(dir.has_entry("file.txt")?);

        // Test 2: Create a directory in root directory
        let subdir = dir.create_entity("subdir", Dir::new(store.clone())).await?;
        assert!(matches!(subdir, Entity::Dir(_)));
        assert!(dir.has_entry("subdir")?);

        // Test 3: Create a file in subdirectory
        let nested_file = dir
            .create_entity("subdir/nested.txt", File::new(store.clone()))
            .await?;
        assert!(matches!(nested_file, Entity::File(_)));
        if let Some(Entity::Dir(subdir)) = dir.find("subdir").await? {
            assert!(subdir.has_entry("nested.txt")?);
        }

        // Test 4: Create a cid symlink
        let target_cid = Cid::default();
        let symlink = dir
            .create_entity("link", SymCidLink::with_cid(store.clone(), target_cid))
            .await?;
        assert!(matches!(symlink, Entity::SymCidLink(_)));
        if let Entity::SymCidLink(link) = symlink {
            assert_eq!(&link.get_cid().await?, &target_cid);
        }

        // Test 5: Attempt to create entity at existing path
        assert!(matches!(
            dir.create_entity("file.txt", File::new(store.clone()))
                .await,
            Err(FsError::PathExists(_))
        ));

        // Test 6: Attempt to create entity in non-existent directory
        assert!(matches!(
            dir.create_entity("nonexistent/file.txt", File::new(store.clone()))
                .await,
            Err(FsError::PathNotFound(_))
        ));

        // Test 7: Attempt to create entity with root path
        assert!(matches!(
            dir.create_entity("/file.txt", File::new(store.clone()))
                .await,
            Err(FsError::PathHasRoot(_))
        ));

        // Test 8: Create multiple entities in nested directories
        dir.create_entity("nested", Dir::new(store.clone())).await?;
        dir.create_entity("nested/dir", Dir::new(store.clone()))
            .await?;
        let deep_file = dir
            .create_entity("nested/dir/file.txt", File::new(store.clone()))
            .await?;
        assert!(matches!(deep_file, Entity::File(_)));

        // Test 9: Verify entity metadata is preserved
        let mut file = File::new(store.clone());
        let content = "test content".as_bytes();
        let content_cid = file.get_store().put_bytes(content).await?;
        file.set_content(Some(content_cid));
        let created_file = dir.create_entity("content.txt", file).await?;
        if let Entity::File(file) = created_file {
            let mut stored_content = Vec::new();
            file.get_store()
                .get_bytes(&content_cid)
                .await?
                .read_to_end(&mut stored_content)
                .await?;
            assert_eq!(stored_content, content);
        }

        // Test 10: Create a path symlink
        let path = "target/path";
        let path_symlink = dir
            .create_entity(
                "path_link",
                SymPathLink::with_path(store.clone(), path.to_string())?,
            )
            .await?;
        assert!(matches!(path_symlink, Entity::SymPathLink(_)));
        if let Entity::SymPathLink(link) = path_symlink {
            assert_eq!(link.get_target_path().as_str(), path);
        }

        Ok(())
    }
}

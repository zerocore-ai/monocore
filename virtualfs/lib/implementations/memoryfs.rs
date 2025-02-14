use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_trait::async_trait;
use getset::Getters;
use tokio::{io::AsyncRead, sync::RwLock};

use crate::{Metadata, ModeType, PathSegment, VfsError, VfsResult, VirtualFileSystem};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An in-memory implementation of a virtual file system.
///
/// This implementation stores all files and directories in memory, making it useful for
/// testing and temporary file systems that don't need persistence.
#[derive(Debug, Clone, Getters)]
#[getset(get = "pub with_prefix")]
pub struct MemoryFileSystem {
    /// The root directory of the file system
    root_dir: Arc<RwLock<Dir>>,
}

/// Represents a directory in the memory file system.
///
/// A directory contains a collection of entries, where each entry is identified by a path segment
/// and can be either another directory, a file, or a symbolic link.
#[derive(Debug, Clone, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Dir {
    /// Metadata associated with the directory
    metadata: Metadata,

    /// Map of path segments to directory entries
    entries: HashMap<PathSegment, Entity>,
}

/// Represents a file in the memory file system.
///
/// A file contains metadata and its content as a byte vector.
#[derive(Debug, Clone, Getters)]
#[getset(get = "pub with_prefix")]
pub struct File {
    /// Metadata associated with the file
    metadata: Metadata,

    /// Content of the file as a byte vector
    content: Vec<u8>,
}

/// Represents a symbolic link in the memory file system.
///
/// A symbolic link contains metadata and points to a target path.
#[derive(Debug, Clone, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Symlink {
    /// Metadata associated with the symlink
    metadata: Metadata,

    /// Target path that the symlink points to
    target: PathBuf,
}

/// Represents an entity in the memory file system.
///
/// An entity can be either a directory, a file, or a symbolic link.
#[derive(Debug, Clone)]
pub enum Entity {
    /// A directory containing other entities
    Dir(Dir),

    /// A file containing data
    File(File),

    /// A symbolic link pointing to another path
    Symlink(Symlink),
}

/// A reader that provides async read access to a memory file's contents
struct MemoryFileReader {
    /// The content to read from
    content: Vec<u8>,

    /// Current position in the content
    position: usize,

    /// Maximum number of bytes to read
    remaining: usize,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MemoryFileSystem {
    /// Creates a new empty memory file system.
    pub fn new() -> Self {
        Self {
            root_dir: Arc::new(RwLock::new(Dir::new())),
        }
    }

    /// Splits the given path into its parent and the last path segment.
    /// If the path has no explicit parent, an empty path is used as the parent.
    #[inline]
    fn split_path(path: &Path) -> VfsResult<(&Path, PathSegment)> {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let name_os = path
            .file_name()
            .ok_or_else(|| VfsError::InvalidPathComponent("No filename provided".into()))?;
        let name = name_os
            .to_str()
            .ok_or_else(|| VfsError::InvalidPathComponent("Invalid filename".into()))?;
        let segment = PathSegment::try_from(name)?;
        Ok((parent, segment))
    }

    /// Given a mutable reference to the current root directory, returns a mutable reference
    /// to the directory corresponding to the provided parent path. If parent is empty, returns the root.
    #[inline]
    fn get_parent_dir<'a>(root: &'a mut Dir, parent: &Path) -> VfsResult<&'a mut Dir> {
        if parent == Path::new("") {
            Ok(root)
        } else {
            match root.find_mut(parent)? {
                Some(entity) => entity.as_mut_dir(),
                None => Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf())),
            }
        }
    }
}

impl File {
    /// Creates a new empty file.
    pub fn new() -> Self {
        Self {
            metadata: Metadata::new(ModeType::File),
            content: Vec::new(),
        }
    }

    /// Creates a new file with the given content.
    ///
    /// ## Arguments
    ///
    /// * `content` - The initial content of the file as a byte vector
    ///
    /// ## Returns
    ///
    /// A new `File` instance with the specified content and default metadata.
    pub fn with_content(content: Vec<u8>) -> Self {
        Self {
            metadata: Metadata::new(ModeType::File),
            content,
        }
    }
}

impl Symlink {
    /// Creates a new symbolic link pointing to the given target.
    ///
    /// ## Arguments
    ///
    /// * `target` - The path that the symlink should point to
    ///
    /// ## Returns
    ///
    /// A new `Symlink` instance with the specified target and default metadata.
    pub fn new(target: PathBuf) -> Self {
        Self {
            metadata: Metadata::new(ModeType::Symlink),
            target,
        }
    }
}

impl Entity {
    /// Attempts to get a reference to this entity as a directory.
    ///
    /// ## Returns
    ///
    /// * `Ok(&Dir)` - If this entity is a directory
    /// * `Err(VfsError::NotADirectory)` - If this entity is not a directory
    pub fn as_dir(&self) -> VfsResult<&Dir> {
        match self {
            Entity::Dir(dir) => Ok(dir),
            _ => Err(VfsError::NotADirectory(PathBuf::new())),
        }
    }

    /// Attempts to get a mutable reference to this entity as a directory.
    ///
    /// ## Returns
    ///
    /// * `Ok(&mut Dir)` - If this entity is a directory
    /// * `Err(VfsError::NotADirectory)` - If this entity is not a directory
    pub fn as_mut_dir(&mut self) -> VfsResult<&mut Dir> {
        match self {
            Entity::Dir(dir) => Ok(dir),
            _ => Err(VfsError::NotADirectory(PathBuf::new())),
        }
    }

    /// Attempts to get a reference to this entity as a file.
    ///
    /// ## Returns
    ///
    /// * `Ok(&File)` - If this entity is a file
    /// * `Err(VfsError::NotAFile)` - If this entity is not a file
    pub fn as_file(&self) -> VfsResult<&File> {
        match self {
            Entity::File(file) => Ok(file),
            _ => Err(VfsError::NotAFile(PathBuf::new())),
        }
    }

    /// Attempts to get a mutable reference to this entity as a file.
    ///
    /// ## Returns
    ///
    /// * `Ok(&mut File)` - If this entity is a file
    /// * `Err(VfsError::NotAFile)` - If this entity is not a file
    pub fn as_mut_file(&mut self) -> VfsResult<&mut File> {
        match self {
            Entity::File(file) => Ok(file),
            _ => Err(VfsError::NotAFile(PathBuf::new())),
        }
    }

    /// Attempts to get a reference to this entity as a symbolic link.
    ///
    /// ## Returns
    ///
    /// * `Ok(&Symlink)` - If this entity is a symbolic link
    /// * `Err(VfsError::NotASymlink)` - If this entity is not a symbolic link
    pub fn as_symlink(&self) -> VfsResult<&Symlink> {
        match self {
            Entity::Symlink(symlink) => Ok(symlink),
            _ => Err(VfsError::NotASymlink(PathBuf::new())),
        }
    }

    /// Attempts to get a mutable reference to this entity as a symbolic link.
    ///
    /// ## Returns
    ///
    /// * `Ok(&mut Symlink)` - If this entity is a symbolic link
    /// * `Err(VfsError::NotASymlink)` - If this entity is not a symbolic link
    pub fn as_mut_symlink(&mut self) -> VfsResult<&mut Symlink> {
        match self {
            Entity::Symlink(symlink) => Ok(symlink),
            _ => Err(VfsError::NotASymlink(PathBuf::new())),
        }
    }
}

impl Dir {
    /// Creates a new empty directory.
    ///
    /// ## Returns
    ///
    /// A new `Dir` instance with no entries and default metadata.
    pub fn new() -> Self {
        Self {
            metadata: Metadata::new(ModeType::Directory),
            entries: HashMap::new(),
        }
    }

    /// Retrieves an entity from the directory's entries using the given path segment.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path segment to look up in the directory entries
    ///
    /// ## Returns
    ///
    /// * `Ok(&Entity)` - A reference to the found entity
    /// * `Err(VfsError::NotFound)` - If no entry exists with the given path segment
    pub fn get(&self, path: &PathSegment) -> Option<&Entity> {
        self.entries.get(path)
    }

    /// Retrieves a mutable reference to an entity from the directory's entries using the given path segment.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path segment to look up in the directory entries
    ///
    /// ## Returns
    ///
    /// * `Ok(&mut Entity)` - A mutable reference to the found entity
    /// * `Err(VfsError::NotFound)` - If no entry exists with the given path segment
    pub fn get_mut(&mut self, path: PathSegment) -> Option<&mut Entity> {
        self.entries.get_mut(&path)
    }

    /// Adds a new entity to the directory's entries with the given path segment.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path segment under which to store the entity
    /// * `entity` - The entity to store
    ///
    /// ## Returns
    ///
    /// * `Ok(())` - If the entity was successfully added
    /// * `Err(VfsError::AlreadyExists)` - If an entry already exists with the given path segment
    pub fn put(&mut self, path: PathSegment, entity: Entity) -> VfsResult<()> {
        if self.entries.contains_key(&path) {
            return Err(VfsError::AlreadyExists(path.into()));
        }
        self.entries.insert(path, entity);
        Ok(())
    }

    /// Traverses a path starting from this directory to find an entity.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path to traverse, can be empty, ".", or a path with normal components
    ///
    /// ## Returns
    ///
    /// * `Ok(&Entity)` - A reference to the found entity
    /// * `Err(VfsError::NotFound)` - If the path doesn't exist
    /// * `Err(VfsError::NotADirectory)` - If a non-final path component isn't a directory
    /// * `Err(VfsError::InvalidPathComponent)` - If the path contains invalid components
    ///
    /// ## Examples
    ///
    /// ```rust,no_run
    /// use std::path::Path;
    /// ## use virtualfs::{Dir, VfsResult};
    ///
    /// ## fn example(dir: Dir) -> VfsResult<()> {
    /// // Find an entity at path "foo/bar"
    /// let entity = dir.find("foo/bar")?;
    /// ## Ok(())
    /// ## }
    /// ```
    pub fn find(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<Option<&Entity>> {
        let path = path.as_ref();

        // Ensure the path is not empty
        let mut components = path.components().peekable();
        if components.peek().is_none() {
            return Err(VfsError::InvalidPathComponent("Empty path provided".into()));
        }

        // Traverse the components
        let mut current_dir = self;

        while let Some(component) = components.next() {
            match component {
                // Only allow normal components
                Component::Normal(os_str) => {
                    // Convert the component to a PathSegment
                    let segment = PathSegment::try_from(os_str.to_str().ok_or_else(|| {
                        VfsError::InvalidPathComponent(os_str.to_string_lossy().into_owned())
                    })?)?;

                    let entry = current_dir.get(&segment);

                    // If this is the last component, return the entry
                    if components.peek().is_none() {
                        return Ok(entry);
                    }

                    // Otherwise, ensure it's a directory and continue traversing
                    match entry {
                        Some(entity) => current_dir = entity.as_dir()?,
                        None => return Ok(None),
                    }
                }
                // Reject all non-normal components, including CurDir (.) and ParentDir (..)
                _ => {
                    return Err(VfsError::InvalidPathComponent(
                        component.as_os_str().to_string_lossy().into_owned(),
                    ))
                }
            }
        }

        unreachable!()
    }

    /// Traverses a path starting from this directory to find an entity, returning a mutable reference.
    ///
    /// This method is similar to `find`, but returns a mutable reference to the found entity.
    /// It follows the same path traversal rules and error handling as `find`.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path to traverse, can be empty, ".", or a path with normal components
    ///
    /// ## Returns
    ///
    /// * `Ok(Some(&mut Entity))` - A mutable reference to the found entity
    /// * `Ok(None)` - If the path doesn't exist
    /// * `Err(VfsError::NotADirectory)` - If a non-final path component isn't a directory
    /// * `Err(VfsError::InvalidPathComponent)` - If the path contains invalid components
    ///
    /// ## Examples
    ///
    /// ```rust,no_run
    /// use std::path::Path;
    /// # use virtualfs::{Dir, VfsResult};
    ///
    /// # fn example(dir: &mut Dir) -> VfsResult<()> {
    /// // Find and modify an entity at path "foo/bar"
    /// if let Some(entity) = dir.find_mut("foo/bar")? {
    ///     // Modify the entity
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn find_mut(
        &mut self,
        path: impl AsRef<Path> + Send + Sync,
    ) -> VfsResult<Option<&mut Entity>> {
        let path = path.as_ref();

        // Ensure the path is not empty
        let mut components = path.components().peekable();
        if components.peek().is_none() {
            return Err(VfsError::InvalidPathComponent("Empty path provided".into()));
        }

        // Traverse the components
        let mut current_dir = self;

        while let Some(component) = components.next() {
            match component {
                // Only allow normal components
                Component::Normal(os_str) => {
                    // Convert the component to a PathSegment
                    let segment = PathSegment::try_from(os_str.to_str().ok_or_else(|| {
                        VfsError::InvalidPathComponent(os_str.to_string_lossy().into_owned())
                    })?)?;

                    let entry = current_dir.get_mut(segment);

                    // If this is the last component, return the entry
                    if components.peek().is_none() {
                        return Ok(entry);
                    }

                    // Otherwise, ensure it's a directory and continue traversing
                    match entry {
                        Some(entity) => current_dir = entity.as_mut_dir()?,
                        None => return Ok(None),
                    }
                }
                // Reject all non-normal components, including CurDir (.) and ParentDir (..)
                _ => {
                    return Err(VfsError::InvalidPathComponent(
                        component.as_os_str().to_string_lossy().into_owned(),
                    ))
                }
            }
        }

        unreachable!()
    }
}

impl MemoryFileReader {
    fn new(content: Vec<u8>, offset: u64, length: u64) -> Self {
        let offset = offset.min(content.len() as u64) as usize;
        let remaining = length.min(content.len() as u64 - offset as u64) as usize;
        let content = content[offset..offset + remaining].to_vec();

        Self {
            content,
            position: 0,
            remaining,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl VirtualFileSystem for MemoryFileSystem {
    async fn exists(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<bool> {
        let path = path.as_ref();
        let result = self.root_dir.read().await.find(path)?.is_some();

        Ok(result)
    }

    async fn create_file(
        &self,
        path: impl AsRef<Path> + Send + Sync,
        exists_ok: bool,
    ) -> VfsResult<()> {
        let path = path.as_ref();
        let (parent, filename) = MemoryFileSystem::split_path(path)?;

        let mut root = self.root_dir.write().await;
        let parent_dir = MemoryFileSystem::get_parent_dir(&mut root, parent)?;

        if let Some(_) = parent_dir.get(&filename) {
            if !exists_ok {
                return Err(VfsError::AlreadyExists(path.to_path_buf()));
            }
            return Ok(());
        }

        parent_dir.put(filename, Entity::File(File::new()))?;

        Ok(())
    }

    async fn create_directory(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<()> {
        let path = path.as_ref();
        let (parent, dirname) = MemoryFileSystem::split_path(path)?;

        let mut root = self.root_dir.write().await;
        let parent_dir = MemoryFileSystem::get_parent_dir(&mut root, parent)?;

        if parent_dir.get(&dirname).is_some() {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        parent_dir.put(dirname, Entity::Dir(Dir::new()))?;

        Ok(())
    }

    async fn create_symlink(
        &self,
        path: impl AsRef<Path> + Send + Sync,
        target: impl AsRef<Path> + Send + Sync,
    ) -> VfsResult<()> {
        let path = path.as_ref();
        let target = target.as_ref();

        if target.as_os_str().is_empty() {
            return Err(VfsError::InvalidSymlinkTarget(target.to_path_buf()));
        }

        let (parent, linkname) = MemoryFileSystem::split_path(path)?;

        let mut root = self.root_dir.write().await;
        let parent_dir = MemoryFileSystem::get_parent_dir(&mut root, parent)?;

        if parent_dir.get(&linkname).is_some() {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        parent_dir.put(
            linkname,
            Entity::Symlink(Symlink::new(target.to_path_buf())),
        )?;

        Ok(())
    }

    async fn read_file(
        &self,
        path: impl AsRef<Path> + Send + Sync,
        offset: u64,
        length: u64,
    ) -> VfsResult<Pin<Box<dyn AsyncRead + Send + Sync + 'static>>> {
        let path = path.as_ref();

        // Find the file
        let root = self.root_dir.read().await;
        let entity = root
            .find(path)?
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))?;

        // Ensure it's a file and get its contents
        let file = entity.as_file()?;
        let content = file.get_content().clone();

        // Create and return the reader
        Ok(Box::pin(MemoryFileReader::new(content, offset, length)))
    }

    async fn read_directory(
        &self,
        path: impl AsRef<Path> + Send + Sync,
    ) -> VfsResult<Box<dyn Iterator<Item = PathSegment> + Send + Sync + 'static>> {
        let path = path.as_ref();

        // Find the directory
        let root = self.root_dir.read().await;
        let entity = if path == Path::new("") {
            Ok(&*root)
        } else {
            root.find(path)?
                .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))?
                .as_dir()
        }?;

        // Clone PathSegments to avoid lifetime issues
        let entries = entity.entries.keys().cloned().collect::<Vec<_>>();

        Ok(Box::new(entries.into_iter()))
    }

    async fn read_symlink(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<PathBuf> {
        let path = path.as_ref();

        // Find the symlink
        let root = self.root_dir.read().await;
        let entity = root
            .find(path)?
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))?;

        // Ensure it's a symlink and get its target
        let symlink = entity.as_symlink()?;
        Ok(symlink.get_target().clone())
    }

    async fn get_metadata(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<Metadata> {
        let path = path.as_ref();

        // Find the entity
        let root = self.root_dir.read().await;
        let entity = root
            .find(path)?
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))?;

        // Return a clone of the metadata based on the entity type
        let metadata = match entity {
            Entity::Dir(dir) => dir.get_metadata().clone(),
            Entity::File(file) => file.get_metadata().clone(),
            Entity::Symlink(symlink) => symlink.get_metadata().clone(),
        };

        Ok(metadata)
    }

    async fn write_file(
        &self,
        path: impl AsRef<Path> + Send + Sync,
        offset: u64,
        data: impl AsyncRead + Send + Sync + 'static,
    ) -> VfsResult<()> {
        let path = path.as_ref();

        // Find the file and get a mutable reference
        let mut root = self.root_dir.write().await;
        let entity = root
            .find_mut(path)?
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))?;

        // Ensure it's a file and get mutable access to its content
        let file = entity.as_mut_file()?;
        let content = &mut file.content;

        // Convert offset to usize, ensuring it's not too large
        let offset = usize::try_from(offset).map_err(|_| VfsError::InvalidOffset {
            path: path.to_path_buf(),
            offset,
        })?;

        // If offset is beyond current size, extend the file with zeros
        if offset > content.len() {
            content.resize(offset, 0);
        }

        // Read all data into a buffer
        let mut buffer = Vec::new();
        let mut pinned_data = Box::pin(data);
        tokio::io::copy(&mut pinned_data, &mut buffer)
            .await
            .map_err(VfsError::Io)?;

        // Ensure the content vector has enough capacity
        let required_len = offset + buffer.len();
        if required_len > content.len() {
            content.resize(required_len, 0);
        }

        // Write the data at the specified offset
        content[offset..offset + buffer.len()].copy_from_slice(&buffer);

        Ok(())
    }

    async fn remove(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<()> {
        let path = path.as_ref();
        let (parent, key) = MemoryFileSystem::split_path(path)?;

        let mut root = self.root_dir.write().await;
        let parent_dir = MemoryFileSystem::get_parent_dir(&mut root, parent)?;

        match parent_dir.get(&key) {
            Some(entity) => match entity {
                Entity::Dir(_) => return Err(VfsError::NotAFile(path.to_path_buf())),
                _ => {
                    parent_dir.entries.remove(&key);
                }
            },
            None => return Err(VfsError::NotFound(path.to_path_buf())),
        };

        Ok(())
    }

    async fn remove_directory(&self, path: impl AsRef<Path> + Send + Sync) -> VfsResult<()> {
        let path = path.as_ref();
        let (parent, dirname) = MemoryFileSystem::split_path(path)?;

        let mut root = self.root_dir.write().await;
        let parent_dir = MemoryFileSystem::get_parent_dir(&mut root, parent)?;

        match parent_dir.get(&dirname) {
            Some(entity) => match entity {
                Entity::Dir(_) => parent_dir.entries.remove(&dirname),
                _ => return Err(VfsError::NotADirectory(path.to_path_buf())),
            },
            None => return Err(VfsError::NotFound(path.to_path_buf())),
        };

        Ok(())
    }

    async fn rename(
        &self,
        old_path: impl AsRef<Path> + Send + Sync,
        new_path: impl AsRef<Path> + Send + Sync,
    ) -> VfsResult<()> {
        let old_path = old_path.as_ref();
        let new_path = new_path.as_ref();

        let (old_parent, old_segment) = MemoryFileSystem::split_path(old_path)?;
        let (new_parent, new_segment) = MemoryFileSystem::split_path(new_path)?;

        {
            let root = self.root_dir.read().await;

            if old_parent != Path::new("") {
                match root.find(old_parent)? {
                    Some(entity) => {
                        if !matches!(entity, Entity::Dir(_)) {
                            return Err(VfsError::NotADirectory(old_parent.to_path_buf()));
                        }
                    }
                    None => {
                        return Err(VfsError::ParentDirectoryNotFound(old_parent.to_path_buf()))
                    }
                }
            }

            if new_parent != Path::new("") {
                match root.find(new_parent)? {
                    Some(entity) => {
                        if !matches!(entity, Entity::Dir(_)) {
                            return Err(VfsError::NotADirectory(new_parent.to_path_buf()));
                        }
                    }
                    None => {
                        return Err(VfsError::ParentDirectoryNotFound(new_parent.to_path_buf()))
                    }
                }
            }

            let source_dir = if old_parent == Path::new("") {
                &root
            } else {
                root.find(old_parent)?.unwrap().as_dir()?
            };

            if !source_dir.entries.contains_key(&old_segment) {
                return Err(VfsError::NotFound(old_path.to_path_buf()));
            }

            let dest_dir = if new_parent == Path::new("") {
                &root
            } else {
                root.find(new_parent)?.unwrap().as_dir()?
            };

            if dest_dir.entries.contains_key(&new_segment) {
                return Err(VfsError::AlreadyExists(new_path.to_path_buf()));
            }
        }

        let mut root = self.root_dir.write().await;

        if old_parent == new_parent {
            let parent_dir = MemoryFileSystem::get_parent_dir(&mut root, old_parent)?;
            let entity = parent_dir.entries.remove(&old_segment).unwrap();
            parent_dir.entries.insert(new_segment, entity);
            return Ok(());
        }

        let entity = if old_parent == Path::new("") {
            root.entries.remove(&old_segment).unwrap()
        } else {
            MemoryFileSystem::get_parent_dir(&mut root, old_parent)?
                .entries
                .remove(&old_segment)
                .unwrap()
        };

        if new_parent == Path::new("") {
            root.entries.insert(new_segment, entity);
        } else {
            MemoryFileSystem::get_parent_dir(&mut root, new_parent)?
                .entries
                .insert(new_segment, entity);
        }

        Ok(())
    }
}

impl Default for MemoryFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Dir {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for File {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncRead for MemoryFileReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let available = self.remaining;
        if available == 0 {
            return Poll::Ready(Ok(()));
        }

        let to_read = buf.remaining().min(available);
        if to_read == 0 {
            return Poll::Ready(Ok(()));
        }

        buf.put_slice(&self.content[self.position..self.position + to_read]);
        self.position += to_read;
        self.remaining -= to_read;

        Poll::Ready(Ok(()))
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memoryfs_dir_new() {
        let dir = Dir::new();
        assert!(dir.entries.is_empty());
        assert_eq!(dir.metadata.get_size(), 0);
    }

    #[test]
    fn test_memoryfs_dir_put_and_get() {
        let mut dir = Dir::new();
        let file = File::with_content(vec![1, 2, 3]);
        let path = PathSegment::try_from("test.txt").unwrap();

        // Test putting a new entry
        assert!(dir.put(path.clone(), Entity::File(file)).is_ok());

        // Test getting the entry
        let entity = dir.get(&path).unwrap();
        match entity {
            Entity::File(f) => assert_eq!(&f.content, &vec![1, 2, 3]),
            _ => panic!("Expected file entity"),
        }

        // Test putting to existing path fails
        assert!(matches!(
            dir.put(path, Entity::File(File::new())),
            Err(VfsError::AlreadyExists(_))
        ));
    }

    #[test]
    fn test_memoryfs_dir_get_nonexistent() {
        let dir = Dir::new();
        let path = PathSegment::try_from("nonexistent.txt").unwrap();
        assert!(dir.get(&path).is_none());
    }

    #[test]
    fn test_memoryfs_dir_find() {
        let mut root = Dir::new();
        let mut subdir = Dir::new();
        let file = File::with_content(vec![1, 2, 3]);

        // Set up directory structure:
        // root/
        //   subdir/
        //     test.txt
        let file_path = PathSegment::try_from("test.txt").unwrap();
        subdir.put(file_path, Entity::File(file)).unwrap();

        let subdir_path = PathSegment::try_from("subdir").unwrap();
        root.put(subdir_path, Entity::Dir(subdir)).unwrap();

        // Test finding file through path
        let found = root.find("subdir/test.txt").unwrap().unwrap();
        match found {
            Entity::File(f) => assert_eq!(&f.content, &vec![1, 2, 3]),
            _ => panic!("Expected file entity"),
        }

        // Test finding directory
        let found = root.find("subdir").unwrap().unwrap();
        assert!(matches!(found, Entity::Dir(_)));

        // Test nonexistent path
        assert!(root.find("nonexistent/path").unwrap().is_none());

        // Test invalid path components
        assert!(matches!(
            root.find(".."),
            Err(VfsError::InvalidPathComponent(_))
        ));
        assert!(matches!(
            root.find("."),
            Err(VfsError::InvalidPathComponent(_))
        ));
        assert!(matches!(
            root.find(""),
            Err(VfsError::InvalidPathComponent(_))
        ));
    }

    #[test]
    fn test_memoryfs_dir_get_mut() {
        let mut dir = Dir::new();
        let file = File::with_content(vec![1, 2, 3]);
        let path = PathSegment::try_from("test.txt").unwrap();

        // Add initial file
        dir.put(path.clone(), Entity::File(file)).unwrap();

        // Get mutable reference and modify
        if let Entity::File(file) = dir.get_mut(path).unwrap() {
            file.content.push(4);
        }

        // Verify modification
        if let Entity::File(file) = dir
            .get(&PathSegment::try_from("test.txt").unwrap())
            .unwrap()
        {
            assert_eq!(&file.content, &vec![1, 2, 3, 4]);
        } else {
            panic!("Expected file entity");
        }
    }

    #[test]
    fn test_memoryfs_dir_find_mut() {
        let mut root = Dir::new();
        let mut subdir = Dir::new();
        let file = File::with_content(vec![1, 2, 3]);

        // Set up directory structure:
        // root/
        //   subdir/
        //     test.txt
        let file_path = PathSegment::try_from("test.txt").unwrap();
        subdir.put(file_path, Entity::File(file)).unwrap();

        let subdir_path = PathSegment::try_from("subdir").unwrap();
        root.put(subdir_path, Entity::Dir(subdir)).unwrap();

        // Test finding and modifying file through path
        let found = root.find_mut("subdir/test.txt").unwrap().unwrap();
        match found {
            Entity::File(f) => {
                assert_eq!(&f.content, &vec![1, 2, 3]);
                f.content.push(4);
            }
            _ => panic!("Expected file entity"),
        }

        // Verify the modification persisted
        let found = root.find("subdir/test.txt").unwrap().unwrap();
        match found {
            Entity::File(f) => assert_eq!(&f.content, &vec![1, 2, 3, 4]),
            _ => panic!("Expected file entity"),
        }

        // Test finding and modifying directory metadata
        let found = root.find_mut("subdir").unwrap().unwrap();
        match found {
            Entity::Dir(d) => {
                d.metadata.set_size(100);
            }
            _ => panic!("Expected directory entity"),
        }

        // Verify the directory modification persisted
        let found = root.find("subdir").unwrap().unwrap();
        match found {
            Entity::Dir(d) => assert_eq!(d.metadata.get_size(), 100),
            _ => panic!("Expected directory entity"),
        }

        // Test nonexistent path
        assert!(root.find_mut("nonexistent/path").unwrap().is_none());

        // Test invalid path components
        assert!(matches!(
            root.find_mut(".."),
            Err(VfsError::InvalidPathComponent(_))
        ));
        assert!(matches!(
            root.find_mut("."),
            Err(VfsError::InvalidPathComponent(_))
        ));
        assert!(matches!(
            root.find_mut(""),
            Err(VfsError::InvalidPathComponent(_))
        ));

        // Test finding and modifying nested file when parent is not a directory
        let file_path = PathSegment::try_from("not_a_dir").unwrap();
        root.put(file_path, Entity::File(File::new())).unwrap();
        assert!(matches!(
            root.find_mut("not_a_dir/file.txt"),
            Err(VfsError::NotADirectory(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_create_file() {
        let fs = MemoryFileSystem::new();

        // Test creating a file in root directory
        assert!(fs.create_file("test.txt", false).await.is_ok());

        // Verify file exists and is a file
        let root = fs.root_dir.read().await;
        let file = root
            .get(&PathSegment::try_from("test.txt").unwrap())
            .unwrap();
        assert!(matches!(file, Entity::File(_)));
        drop(root);

        // Test creating file when parent directory doesn't exist
        assert!(matches!(
            fs.create_file("nonexistent/test.txt", false).await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));

        // Test exists_ok behavior
        assert!(matches!(
            fs.create_file("test.txt", false).await,
            Err(VfsError::AlreadyExists(_))
        ));
        assert!(fs.create_file("test.txt", true).await.is_ok());

        // Test creating file in subdirectory
        {
            // First create the directory
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("subdir").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
            drop(root);

            // Then create file in it
            assert!(fs.create_file("subdir/nested.txt", false).await.is_ok());

            // Verify nested file exists
            let root = fs.root_dir.read().await;
            let subdir = root.find("subdir").unwrap().unwrap().as_dir().unwrap();
            let file = subdir
                .get(&PathSegment::try_from("nested.txt").unwrap())
                .unwrap();
            assert!(matches!(file, Entity::File(_)));
        }

        // Test invalid path components
        for invalid_path in [".", "..", "/"] {
            assert!(matches!(
                fs.create_file(invalid_path, false).await,
                Err(VfsError::InvalidPathComponent(_))
            ));
        }

        // Test concurrent file creation
        let fs2 = fs.clone();
        let handle1 = tokio::spawn(async move { fs.create_file("concurrent.txt", false).await });
        let handle2 = tokio::spawn(async move { fs2.create_file("concurrent.txt", false).await });

        let (result1, result2) = tokio::join!(handle1, handle2);
        let results = vec![result1.unwrap(), result2.unwrap()];
        assert!(results.iter().filter(|r| r.is_ok()).count() == 1);
        assert!(
            results
                .iter()
                .filter(|r| matches!(r, Err(VfsError::AlreadyExists(_))))
                .count()
                == 1
        );
    }

    #[tokio::test]
    async fn test_memoryfs_create_directory() {
        let fs = MemoryFileSystem::new();

        // Test creating a directory in root
        assert!(fs.create_directory("testdir").await.is_ok());

        // Verify directory exists and is a directory
        let root = fs.root_dir.read().await;
        let dir = root
            .get(&PathSegment::try_from("testdir").unwrap())
            .unwrap();
        assert!(matches!(dir, Entity::Dir(_)));
        drop(root);

        // Test creating directory when parent doesn't exist
        assert!(matches!(
            fs.create_directory("nonexistent/subdir").await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));

        // Test creating existing directory fails
        assert!(matches!(
            fs.create_directory("testdir").await,
            Err(VfsError::AlreadyExists(_))
        ));

        // Test creating nested directory
        {
            // Create parent directory first
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("parent").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
            drop(root);

            // Create nested directory
            assert!(fs.create_directory("parent/child").await.is_ok());

            // Verify nested directory exists
            let root = fs.root_dir.read().await;
            let parent = root.find("parent").unwrap().unwrap().as_dir().unwrap();
            let child = parent
                .get(&PathSegment::try_from("child").unwrap())
                .unwrap();
            assert!(matches!(child, Entity::Dir(_)));
        }

        // Test invalid path components
        for invalid_path in [".", "..", "/"] {
            assert!(matches!(
                fs.create_directory(invalid_path).await,
                Err(VfsError::InvalidPathComponent(_))
            ));
        }

        // Test concurrent directory creation
        let fs2 = fs.clone();
        let handle1 = tokio::spawn(async move { fs.create_directory("concurrent").await });
        let handle2 = tokio::spawn(async move { fs2.create_directory("concurrent").await });

        let (result1, result2) = tokio::join!(handle1, handle2);
        let results = vec![result1.unwrap(), result2.unwrap()];
        assert!(results.iter().filter(|r| r.is_ok()).count() == 1);
        assert!(
            results
                .iter()
                .filter(|r| matches!(r, Err(VfsError::AlreadyExists(_))))
                .count()
                == 1
        );
    }

    #[tokio::test]
    async fn test_memoryfs_create_symlink() {
        let fs = MemoryFileSystem::new();

        // Test creating a symlink in root
        assert!(fs.create_symlink("link", "target").await.is_ok());

        // Verify symlink exists and points to correct target
        let root = fs.root_dir.read().await;
        let link = root.get(&PathSegment::try_from("link").unwrap()).unwrap();
        match link {
            Entity::Symlink(symlink) => assert_eq!(symlink.get_target(), &PathBuf::from("target")),
            _ => panic!("Expected symlink entity"),
        }
        drop(root);

        // Test creating symlink when parent doesn't exist
        assert!(matches!(
            fs.create_symlink("nonexistent/link", "target").await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));

        // Test creating existing symlink fails
        assert!(matches!(
            fs.create_symlink("link", "new_target").await,
            Err(VfsError::AlreadyExists(_))
        ));

        // Test creating symlink with empty target
        assert!(matches!(
            fs.create_symlink("invalid", "").await,
            Err(VfsError::InvalidSymlinkTarget(_))
        ));

        // Test creating nested symlink
        {
            // Create parent directory first
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("parent").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
            drop(root);

            // Create nested symlink
            assert!(fs.create_symlink("parent/link", "../target").await.is_ok());

            // Verify nested symlink exists and points to correct target
            let root = fs.root_dir.read().await;
            let parent = root.find("parent").unwrap().unwrap().as_dir().unwrap();
            let link = parent.get(&PathSegment::try_from("link").unwrap()).unwrap();
            match link {
                Entity::Symlink(symlink) => {
                    assert_eq!(symlink.get_target(), &PathBuf::from("../target"))
                }
                _ => panic!("Expected symlink entity"),
            }
        }

        // Test invalid path components
        for invalid_path in [".", "..", "/"] {
            assert!(matches!(
                fs.create_symlink(invalid_path, "target").await,
                Err(VfsError::InvalidPathComponent(_))
            ));
        }

        // Test concurrent symlink creation
        let fs2 = fs.clone();
        let handle1 = tokio::spawn(async move { fs.create_symlink("concurrent", "target1").await });
        let handle2 =
            tokio::spawn(async move { fs2.create_symlink("concurrent", "target2").await });

        let (result1, result2) = tokio::join!(handle1, handle2);
        let results = vec![result1.unwrap(), result2.unwrap()];
        assert!(results.iter().filter(|r| r.is_ok()).count() == 1);
        assert!(
            results
                .iter()
                .filter(|r| matches!(r, Err(VfsError::AlreadyExists(_))))
                .count()
                == 1
        );
    }

    #[tokio::test]
    async fn test_memoryfs_read_file() {
        let fs = MemoryFileSystem::new();
        let content = vec![1, 2, 3, 4, 5];

        // Create a test file
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("test.bin").unwrap(),
                Entity::File(File::with_content(content.clone())),
            )
            .unwrap();
        }

        // Test reading entire file
        let mut reader = fs.read_file("test.bin", 0, 5).await.unwrap();
        let mut buf = Vec::new();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, content);

        // Test reading with offset
        let mut reader = fs.read_file("test.bin", 2, 2).await.unwrap();
        let mut buf = Vec::new();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, vec![3, 4]);

        // Test reading beyond file size
        let mut reader = fs.read_file("test.bin", 4, 10).await.unwrap();
        let mut buf = Vec::new();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, vec![5]);

        // Test reading non-existent file
        assert!(matches!(
            fs.read_file("nonexistent", 0, 1).await,
            Err(VfsError::NotFound(_))
        ));

        // Test reading directory as file
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("dir").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
        }
        assert!(matches!(
            fs.read_file("dir", 0, 1).await,
            Err(VfsError::NotAFile(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_read_directory() {
        let fs = MemoryFileSystem::new();

        // Create test directory structure
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("file1.txt").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();
            root.put(
                PathSegment::try_from("file2.txt").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();

            let mut subdir = Dir::new();
            subdir
                .put(
                    PathSegment::try_from("nested.txt").unwrap(),
                    Entity::File(File::new()),
                )
                .unwrap();
            root.put(
                PathSegment::try_from("subdir").unwrap(),
                Entity::Dir(subdir),
            )
            .unwrap();
        }

        // Test reading root directory
        let entries: Vec<_> = fs.read_directory("").await.unwrap().collect();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&PathSegment::try_from("file1.txt").unwrap()));
        assert!(entries.contains(&PathSegment::try_from("file2.txt").unwrap()));
        assert!(entries.contains(&PathSegment::try_from("subdir").unwrap()));

        // Test reading subdirectory
        let entries: Vec<_> = fs.read_directory("subdir").await.unwrap().collect();
        assert_eq!(entries.len(), 1);
        assert!(entries.contains(&PathSegment::try_from("nested.txt").unwrap()));

        // Test reading non-existent directory
        assert!(matches!(
            fs.read_directory("nonexistent").await,
            Err(VfsError::NotFound(_))
        ));

        // Test reading file as directory
        assert!(matches!(
            fs.read_directory("file1.txt").await,
            Err(VfsError::NotADirectory(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_read_symlink() {
        let fs = MemoryFileSystem::new();

        // Create test symlink
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("link").unwrap(),
                Entity::Symlink(Symlink::new(PathBuf::from("target"))),
            )
            .unwrap();
        }

        // Test reading symlink
        let target = fs.read_symlink("link").await.unwrap();
        assert_eq!(target, PathBuf::from("target"));

        // Test reading non-existent symlink
        assert!(matches!(
            fs.read_symlink("nonexistent").await,
            Err(VfsError::NotFound(_))
        ));

        // Test reading file as symlink
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("file").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();
        }
        assert!(matches!(
            fs.read_symlink("file").await,
            Err(VfsError::NotASymlink(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_write_file() {
        let fs = MemoryFileSystem::new();

        // Create an empty file
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("test.txt").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();
        }

        // Test writing to file
        let data = b"Hello, World!".to_vec();
        let reader = std::io::Cursor::new(data.clone());
        assert!(fs.write_file("test.txt", 0, reader).await.is_ok());

        // Verify content
        let mut buf = Vec::new();
        let mut reader = fs
            .read_file("test.txt", 0, data.len() as u64)
            .await
            .unwrap();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, data.clone());

        // Test writing with offset
        let append_data = b", Rust!".to_vec();
        let reader = std::io::Cursor::new(append_data.clone());
        let data_len = data.len() as u64;
        assert!(fs.write_file("test.txt", data_len, reader).await.is_ok());

        // Verify appended content
        let mut buf = Vec::new();
        let total_len = data_len + append_data.len() as u64;
        let mut reader = fs.read_file("test.txt", 0, total_len).await.unwrap();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        let mut expected = data.clone();
        expected.extend_from_slice(&append_data);
        assert_eq!(buf, expected);

        // Test writing with gap (offset beyond current size)
        let gap_data = b"gap".to_vec();
        let reader = std::io::Cursor::new(gap_data.clone());
        let gap_offset = total_len + 5;
        assert!(fs.write_file("test.txt", gap_offset, reader).await.is_ok());

        // Verify content with gap (should be filled with zeros)
        let mut buf = Vec::new();
        let final_len = gap_offset + gap_data.len() as u64;
        let mut reader = fs.read_file("test.txt", 0, final_len).await.unwrap();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        expected.extend_from_slice(&vec![0; 5]);
        expected.extend_from_slice(&gap_data);
        assert_eq!(buf, expected);

        // Test writing to non-existent file
        let reader = std::io::Cursor::new(vec![1, 2, 3]);
        assert!(matches!(
            fs.write_file("nonexistent", 0, reader).await,
            Err(VfsError::NotFound(_))
        ));

        // Test writing to directory
        {
            let mut root = fs.root_dir.write().await;
            root.put(
                PathSegment::try_from("dir").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
        }
        let reader = std::io::Cursor::new(vec![1, 2, 3]);
        assert!(matches!(
            fs.write_file("dir", 0, reader).await,
            Err(VfsError::NotAFile(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_get_metadata() {
        let fs = MemoryFileSystem::new();

        // Create test entities
        {
            let mut root = fs.root_dir.write().await;
            // Create a file
            root.put(
                PathSegment::try_from("file.txt").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();
            // Create a directory
            root.put(
                PathSegment::try_from("dir").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
            // Create a symlink
            root.put(
                PathSegment::try_from("link").unwrap(),
                Entity::Symlink(Symlink::new(PathBuf::from("target"))),
            )
            .unwrap();
        }

        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                // Test getting file metadata
                let metadata = fs.get_metadata("file.txt").await.unwrap();
                assert_eq!(metadata.get_mode().get_type(), Some(ModeType::File));

                // Test getting directory metadata
                let metadata = fs.get_metadata("dir").await.unwrap();
                assert_eq!(metadata.get_mode().get_type(), Some(ModeType::Directory));

                // Test getting symlink metadata
                let metadata = fs.get_metadata("link").await.unwrap();
                assert_eq!(metadata.get_mode().get_type(), Some(ModeType::Symlink));
            }
        }

        // Test getting metadata for non-existent path
        assert!(matches!(
            fs.get_metadata("nonexistent").await,
            Err(VfsError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_remove() {
        let fs = MemoryFileSystem::new();

        // Create test files and directories
        {
            let mut root = fs.root_dir.write().await;
            // Create a file
            root.put(
                PathSegment::try_from("file.txt").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();
            // Create a directory
            root.put(
                PathSegment::try_from("dir").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
            // Create a symlink
            root.put(
                PathSegment::try_from("link").unwrap(),
                Entity::Symlink(Symlink::new(PathBuf::from("target"))),
            )
            .unwrap();
            // Create a nested file
            let mut subdir = Dir::new();
            subdir
                .put(
                    PathSegment::try_from("nested.txt").unwrap(),
                    Entity::File(File::new()),
                )
                .unwrap();
            root.put(
                PathSegment::try_from("subdir").unwrap(),
                Entity::Dir(subdir),
            )
            .unwrap();
        }

        // Test removing a file
        assert!(fs.remove("file.txt").await.is_ok());
        assert!(matches!(
            fs.read_file("file.txt", 0, 1).await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing a symlink
        assert!(fs.remove("link").await.is_ok());
        assert!(matches!(
            fs.read_symlink("link").await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing a directory (should fail)
        assert!(matches!(fs.remove("dir").await, Err(VfsError::NotAFile(_))));

        // Test removing a nested file
        assert!(fs.remove("subdir/nested.txt").await.is_ok());
        assert!(matches!(
            fs.read_file("subdir/nested.txt", 0, 1).await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing non-existent file
        assert!(matches!(
            fs.remove("nonexistent").await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing from non-existent parent directory
        assert!(matches!(
            fs.remove("nonexistent/file.txt").await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_remove_directory() {
        let fs = MemoryFileSystem::new();

        // Create test directory structure
        {
            let mut root = fs.root_dir.write().await;
            // Create an empty directory
            root.put(
                PathSegment::try_from("empty_dir").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();
            // Create a file
            root.put(
                PathSegment::try_from("file.txt").unwrap(),
                Entity::File(File::new()),
            )
            .unwrap();
            // Create a nested directory structure
            let mut subdir = Dir::new();
            let mut nested = Dir::new();
            nested
                .put(
                    PathSegment::try_from("deep.txt").unwrap(),
                    Entity::File(File::new()),
                )
                .unwrap();
            subdir
                .put(
                    PathSegment::try_from("nested").unwrap(),
                    Entity::Dir(nested),
                )
                .unwrap();
            root.put(
                PathSegment::try_from("subdir").unwrap(),
                Entity::Dir(subdir),
            )
            .unwrap();
        }

        // Test removing an empty directory
        assert!(fs.remove_directory("empty_dir").await.is_ok());
        assert!(matches!(
            fs.read_directory("empty_dir").await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing a file as directory (should fail)
        assert!(matches!(
            fs.remove_directory("file.txt").await,
            Err(VfsError::NotADirectory(_))
        ));

        // Test removing a nested directory
        assert!(fs.remove_directory("subdir/nested").await.is_ok());
        let entries: Vec<_> = fs.read_directory("subdir").await.unwrap().collect();
        assert!(entries.is_empty());

        // Test removing a directory with parent path
        assert!(fs.remove_directory("subdir").await.is_ok());
        assert!(matches!(
            fs.read_directory("subdir").await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing non-existent directory
        assert!(matches!(
            fs.remove_directory("nonexistent").await,
            Err(VfsError::NotFound(_))
        ));

        // Test removing from non-existent parent directory
        assert!(matches!(
            fs.remove_directory("nonexistent/dir").await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_memoryfs_rename() {
        let fs = MemoryFileSystem::new();

        // Create test directory structure
        {
            let mut root = fs.root_dir.write().await;

            // Create files in root
            root.put(
                PathSegment::try_from("file.txt").unwrap(),
                Entity::File(File::with_content(vec![1, 2, 3])),
            )
            .unwrap();

            // Create directories
            root.put(
                PathSegment::try_from("dir1").unwrap(),
                Entity::Dir(Dir::new()),
            )
            .unwrap();

            let mut dir2 = Dir::new();
            dir2.put(
                PathSegment::try_from("nested.txt").unwrap(),
                Entity::File(File::with_content(vec![4, 5, 6])),
            )
            .unwrap();
            root.put(PathSegment::try_from("dir2").unwrap(), Entity::Dir(dir2))
                .unwrap();

            // Create symlink
            root.put(
                PathSegment::try_from("link").unwrap(),
                Entity::Symlink(Symlink::new(PathBuf::from("target"))),
            )
            .unwrap();
        }

        // Test 1: Rename file in same directory
        assert!(fs.rename("file.txt", "renamed.txt").await.is_ok());
        assert!(fs.exists("renamed.txt").await.unwrap());
        assert!(!fs.exists("file.txt").await.unwrap());

        // Verify content is preserved
        let mut buf = Vec::new();
        let mut reader = fs.read_file("renamed.txt", 0, 3).await.unwrap();
        tokio::io::copy(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, vec![1, 2, 3]);

        // Test 2: Move file to different directory
        assert!(fs.rename("renamed.txt", "dir1/moved.txt").await.is_ok());
        assert!(fs.exists("dir1/moved.txt").await.unwrap());
        assert!(!fs.exists("renamed.txt").await.unwrap());

        // Test 3: Move nested file to root
        assert!(fs.rename("dir2/nested.txt", "extracted.txt").await.is_ok());
        assert!(fs.exists("extracted.txt").await.unwrap());
        assert!(!fs.exists("dir2/nested.txt").await.unwrap());

        // Test 4: Rename symlink
        assert!(fs.rename("link", "newlink").await.is_ok());
        assert_eq!(
            fs.read_symlink("newlink").await.unwrap(),
            PathBuf::from("target")
        );
        assert!(!fs.exists("link").await.unwrap());

        // Test 5: Move between directories
        assert!(fs.rename("dir1/moved.txt", "dir2/final.txt").await.is_ok());
        assert!(fs.exists("dir2/final.txt").await.unwrap());
        assert!(!fs.exists("dir1/moved.txt").await.unwrap());

        // Error cases

        // Test 6: Source doesn't exist
        assert!(matches!(
            fs.rename("nonexistent", "dest.txt").await,
            Err(VfsError::NotFound(_))
        ));

        // Test 7: Destination already exists
        assert!(fs.create_file("existing.txt", false).await.is_ok());
        assert!(matches!(
            fs.rename("extracted.txt", "existing.txt").await,
            Err(VfsError::AlreadyExists(_))
        ));

        // Test 8: Source parent directory doesn't exist
        assert!(matches!(
            fs.rename("nonexistent_dir/file.txt", "dest.txt").await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));

        // Test 9: Destination parent directory doesn't exist
        assert!(matches!(
            fs.rename("extracted.txt", "nonexistent_dir/file.txt").await,
            Err(VfsError::ParentDirectoryNotFound(_))
        ));

        // Test 10: Source parent is not a directory
        assert!(fs.create_file("not_a_dir", false).await.is_ok());
        assert!(matches!(
            fs.rename("not_a_dir/file.txt", "dest.txt").await,
            Err(VfsError::NotADirectory(_))
        ));

        // Test 11: Destination parent is not a directory
        assert!(matches!(
            fs.rename("extracted.txt", "not_a_dir/file.txt").await,
            Err(VfsError::NotADirectory(_))
        ));

        // Test 12: Invalid path components
        for invalid_path in [".", "..", "/"] {
            assert!(matches!(
                fs.rename("extracted.txt", invalid_path).await,
                Err(VfsError::InvalidPathComponent(_))
            ));
            assert!(matches!(
                fs.rename(invalid_path, "valid.txt").await,
                Err(VfsError::InvalidPathComponent(_))
            ));
        }

        // Test 13: Concurrent rename operations
        let fs2 = fs.clone();
        let handle1 =
            tokio::spawn(async move { fs.rename("extracted.txt", "concurrent1.txt").await });
        let handle2 =
            tokio::spawn(async move { fs2.rename("extracted.txt", "concurrent2.txt").await });

        let (result1, result2) = tokio::join!(handle1, handle2);
        let results = vec![result1.unwrap(), result2.unwrap()];
        assert!(results.iter().filter(|r| r.is_ok()).count() == 1);
        assert!(
            results
                .iter()
                .filter(|r| matches!(r, Err(VfsError::NotFound(_))))
                .count()
                == 1
        );
    }
}

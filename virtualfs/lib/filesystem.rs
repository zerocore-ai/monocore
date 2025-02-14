use crate::{Metadata, PathSegment, VfsResult};

use std::{
    path::{Path, PathBuf},
    pin::Pin,
};

use async_trait::async_trait;
use tokio::io::AsyncRead;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait that defines the interface for a virtual file system implementation.
///
/// The `VirtualFileSystem` trait provides a set of asynchronous operations for interacting with
/// files and directories in an abstract file system. This abstraction allows for different
/// implementations such as in-memory filesystems, overlay filesystems, or traditional disk-based
/// filesystems while maintaining a consistent interface.
#[async_trait]
pub trait VirtualFileSystem {
    /// Checks if a file or directory exists at the specified path.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path to check for existence
    ///
    /// ## Returns
    ///
    /// * `Ok(true)` if the path exists
    /// * `Ok(false)` if the path does not exist
    /// * `Err` if the check operation fails
    async fn exists(&self, path: &Path) -> VfsResult<bool>;

    /// Creates a new empty file at the specified path.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path where the file should be created
    /// * `exists_ok` - If true, does not error when the file already exists
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The parent directory doesn't exist
    /// - The file already exists and `exists_ok` is false
    async fn create_file(&self, path: &Path, exists_ok: bool) -> VfsResult<()>;

    /// Creates a new directory at the specified path.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path where the directory should be created
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The parent directory doesn't exist
    /// - A file or directory already exists at the path
    async fn create_directory(&self, path: &Path) -> VfsResult<()>;

    /// Creates a symbolic link at the specified path pointing to the target.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path where the symlink should be created
    /// * `target` - The path that the symlink should point to
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The parent directory doesn't exist
    /// - A file or directory already exists at the path
    /// - The target is invalid
    async fn create_symlink(&self, path: &Path, target: &Path) -> VfsResult<()>;

    /// Reads data from a file starting at the specified offset.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the file to read from
    /// * `offset` - The byte offset where reading should start
    /// * `length` - The number of bytes to read
    ///
    /// ## Returns
    ///
    /// Returns a pinned `AsyncRead` implementation that can be used to read the file data.
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The file doesn't exist
    /// - The offset is beyond the end of the file
    async fn read_file(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
    ) -> VfsResult<Pin<Box<dyn AsyncRead + Send + Sync + 'static>>>;

    /// Lists the contents of a directory.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the directory to read
    ///
    /// ## Returns
    ///
    /// Returns an iterator over the paths of entries in the directory.
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The path doesn't exist
    /// - The path is not a directory
    async fn read_directory(
        &self,
        path: &Path,
    ) -> VfsResult<Box<dyn Iterator<Item = PathSegment> + Send + Sync + 'static>>;

    /// Reads the target of a symbolic link.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the symlink to read
    ///
    /// ## Returns
    ///
    /// Returns the target path of the symlink.
    async fn read_symlink(&self, path: &Path) -> VfsResult<PathBuf>;

    /// Gets the metadata of a file or directory.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the file or directory to get metadata for
    ///
    /// ## Returns
    ///
    /// Returns the metadata of the file or directory.
    async fn get_metadata(&self, path: &Path) -> VfsResult<Metadata>;

    /// Sets the metadata of a file or directory.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the file or directory to set metadata for
    /// * `metadata` - The new metadata for the file or directory
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The path doesn't exist
    /// - The path is not a file or directory
    async fn set_metadata(&self, path: &Path, metadata: Metadata) -> VfsResult<()>;

    /// Writes data to a file starting at the specified offset.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the file to write to
    /// * `offset` - The byte offset where writing should start
    /// * `data` - An `AsyncRead` implementation providing the data to write
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The file doesn't exist
    /// - The offset is invalid
    async fn write_file(
        &self,
        path: &Path,
        offset: u64,
        data: Pin<Box<dyn AsyncRead + Send + Sync + 'static>>,
    ) -> VfsResult<()>;

    /// Removes a file from the filesystem.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the file to remove
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The path doesn't exist
    /// - The path is a directory (use `remove_directory` instead)
    async fn remove(&self, path: &Path) -> VfsResult<()>;

    /// Renames (moves) a file or directory to a new location.
    ///
    /// ## Arguments
    ///
    /// * `old_path` - The current path of the file or directory
    /// * `new_path` - The desired new path
    ///
    /// ## Errors
    ///
    /// Returns an error if:
    /// - The source path doesn't exist
    /// - The destination path already exists
    /// - The parent directory of the destination doesn't exist
    async fn rename(&self, old_path: &Path, new_path: &Path) -> VfsResult<()>;
}

use std::{
    io::{self, SeekFrom},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    pin::Pin,
};

use async_trait::async_trait;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncRead, AsyncSeekExt, ReadBuf},
};

#[cfg(unix)]
use crate::metadata::Mode;
use crate::{Metadata, ModeType, PathSegment, VfsError, VfsResult, VirtualFileSystem};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A filesystem implementation that uses the native filesystem.
///
/// This implementation provides direct access to the underlying filesystem, rooted at a specific
/// directory. All operations are performed relative to this root directory.
#[derive(Debug, Clone)]
pub struct NativeFileSystem {
    /// The root directory for this filesystem instance
    root_path: PathBuf,
}

//--------------------------------------------------------------------------------------------------
// Implementation
//--------------------------------------------------------------------------------------------------

impl NativeFileSystem {
    /// Creates a new native filesystem with the given root path.
    ///
    /// ## Arguments
    ///
    /// * `root_path` - The root path for this filesystem instance
    ///
    /// ## Returns
    ///
    /// A new `NativeFileSystem` instance rooted at the specified path.
    pub fn new(root_path: PathBuf) -> Self {
        Self { root_path }
    }

    /// Converts a virtual path to a native filesystem path.
    ///
    /// This method combines the root path with the provided virtual path
    /// to create the actual filesystem path that should be accessed.
    ///
    /// ## Arguments
    ///
    /// * `path` - The virtual path to convert
    ///
    /// ## Returns
    ///
    /// The corresponding native filesystem path
    fn to_native_path(&self, path: &Path) -> PathBuf {
        if path == Path::new("") {
            self.root_path.clone()
        } else {
            self.root_path.join(path)
        }
    }

    /// Gets metadata for a path, returning an error if the path doesn't exist.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path to get metadata for
    ///
    /// ## Returns
    ///
    /// The metadata if the path exists, or an appropriate error
    async fn symlink_metadata_checked(&self, path: &Path) -> VfsResult<std::fs::Metadata> {
        match tokio::fs::symlink_metadata(path).await {
            Ok(m) => Ok(m),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(VfsError::NotFound(path.to_path_buf()))
            }
            Err(e) => Err(VfsError::Io(e)),
        }
    }

    /// Gets metadata for a path, returning None if the path doesn't exist.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path to get metadata for
    ///
    /// ## Returns
    ///
    /// The metadata wrapped in Some if the path exists, None if it doesn't exist,
    /// or an error for other failure cases
    async fn symlink_metadata_option(&self, path: &Path) -> VfsResult<Option<std::fs::Metadata>> {
        match tokio::fs::symlink_metadata(path).await {
            Ok(m) => Ok(Some(m)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(VfsError::Io(e)),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl VirtualFileSystem for NativeFileSystem {
    async fn exists(&self, path: &Path) -> VfsResult<bool> {
        let native_path = self.to_native_path(path);
        Ok(self.symlink_metadata_option(&native_path).await?.is_some())
    }

    async fn create_file(&self, path: &Path, exists_ok: bool) -> VfsResult<()> {
        let native_path = self.to_native_path(path);

        let file_exists = self.symlink_metadata_option(&native_path).await?;
        if file_exists.is_some() && !exists_ok {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        if let Some(parent) = native_path.parent() {
            match self.symlink_metadata_checked(parent).await {
                Ok(_) => {}
                Err(VfsError::NotFound(_)) => {
                    return Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf()))
                }
                Err(e) => return Err(e),
            }
        }

        OpenOptions::new()
            .write(true)
            .create(true)
            .open(native_path)
            .await
            .map_err(VfsError::Io)?;

        Ok(())
    }

    async fn create_directory(&self, path: &Path) -> VfsResult<()> {
        let native_path = self.to_native_path(path);

        if self.symlink_metadata_option(&native_path).await?.is_some() {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        if let Some(parent) = native_path.parent() {
            match self.symlink_metadata_checked(parent).await {
                Ok(_) => {}
                Err(VfsError::NotFound(_)) => {
                    return Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf()))
                }
                Err(e) => return Err(e),
            }
        }

        tokio::fs::create_dir(native_path)
            .await
            .map_err(VfsError::Io)
    }

    async fn create_symlink(&self, path: &Path, target: &Path) -> VfsResult<()> {
        let native_path = self.to_native_path(path);
        let native_target = self.to_native_path(target);

        if self.symlink_metadata_option(&native_path).await?.is_some() {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        if let Some(parent) = native_path.parent() {
            match self.symlink_metadata_checked(parent).await {
                Ok(_) => {}
                Err(VfsError::NotFound(_)) => {
                    return Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf()))
                }
                Err(e) => return Err(e),
            }
        }

        #[cfg(unix)]
        {
            tokio::fs::symlink(native_target, native_path)
                .await
                .map_err(VfsError::Io)
        }

        #[cfg(windows)]
        {
            if native_target.is_dir() {
                tokio::fs::symlink_dir(native_target, native_path)
                    .await
                    .map_err(VfsError::Io)
            } else {
                tokio::fs::symlink_file(native_target, native_path)
                    .await
                    .map_err(VfsError::Io)
            }
        }
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
    ) -> VfsResult<Pin<Box<dyn AsyncRead + Send + Sync + 'static>>> {
        let native_path = self.to_native_path(path);

        let meta = self.symlink_metadata_checked(&native_path).await?;
        if !meta.is_file() {
            return Err(VfsError::NotAFile(path.to_path_buf()));
        }

        let mut file = File::open(native_path).await.map_err(VfsError::Io)?;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(VfsError::Io)?;

        // Create a wrapper that limits reading to the specified length
        struct LimitedReader {
            inner: File,
            remaining: u64,
        }

        impl AsyncRead for LimitedReader {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut ReadBuf<'_>,
            ) -> std::task::Poll<io::Result<()>> {
                if self.remaining == 0 {
                    return std::task::Poll::Ready(Ok(()));
                }

                let max_read = buf.remaining().min(self.remaining as usize);
                let mut limited_buf = ReadBuf::new(buf.initialize_unfilled_to(max_read));

                let poll = Pin::new(&mut self.inner).poll_read(cx, &mut limited_buf);

                if let std::task::Poll::Ready(Ok(())) = poll {
                    let filled = limited_buf.filled().len();
                    buf.advance(filled);
                    self.remaining -= filled as u64;
                }

                poll
            }
        }

        Ok(Box::pin(LimitedReader {
            inner: file,
            remaining: length,
        }))
    }

    async fn read_directory(
        &self,
        path: &Path,
    ) -> VfsResult<Box<dyn Iterator<Item = PathSegment> + Send + Sync + 'static>> {
        let native_path = self.to_native_path(path);

        let meta = self.symlink_metadata_checked(&native_path).await?;
        if !meta.is_dir() {
            return Err(VfsError::NotADirectory(path.to_path_buf()));
        }

        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&native_path)
            .await
            .map_err(VfsError::Io)?;

        while let Some(entry) = dir.next_entry().await.map_err(VfsError::Io)? {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(segment) = PathSegment::try_from(name) {
                    entries.push(segment);
                }
            }
        }

        Ok(Box::new(entries.into_iter()))
    }

    async fn read_symlink(&self, path: &Path) -> VfsResult<PathBuf> {
        let native_path = self.to_native_path(path);

        let meta = self.symlink_metadata_checked(&native_path).await?;
        if !meta.file_type().is_symlink() {
            return Err(VfsError::NotASymlink(path.to_path_buf()));
        }

        tokio::fs::read_link(native_path)
            .await
            .map_err(VfsError::Io)
            .map(|target| {
                if target.is_absolute() {
                    if let Ok(relative) = target.strip_prefix(&self.root_path) {
                        relative.to_path_buf()
                    } else {
                        target
                    }
                } else {
                    target
                }
            })
    }

    async fn get_metadata(&self, path: &Path) -> VfsResult<Metadata> {
        let native_path = self.to_native_path(path);
        let metadata = self.symlink_metadata_checked(&native_path).await?;

        #[cfg(unix)]
        {
            let mode_type = if metadata.is_dir() {
                ModeType::Directory
            } else if metadata.is_file() {
                ModeType::File
            } else if metadata.is_symlink() {
                ModeType::Symlink
            } else {
                ModeType::File
            };

            let mut vfs_metadata = Metadata::new(mode_type);
            vfs_metadata.set_size(metadata.len());

            let native_mode = metadata.permissions().mode();
            // Override the default permissions with the actual native permissions
            vfs_metadata.set_permissions(Mode::from(native_mode).get_permissions());

            Ok(vfs_metadata)
        }

        #[cfg(not(unix))]
        {
            let mut vfs_metadata = Metadata::new(metadata.entity_type());
            vfs_metadata.set_size(metadata.len());

            Ok(vfs_metadata)
        }
    }

    async fn set_metadata(&self, path: &Path, metadata: Metadata) -> VfsResult<()> {
        let native_path = self.to_native_path(path);

        self.symlink_metadata_checked(&native_path).await?;

        #[cfg(unix)]
        {
            let mode: u32 = (*metadata.get_mode()).into();
            let mut perms = tokio::fs::symlink_metadata(&native_path)
                .await
                .map_err(VfsError::Io)?
                .permissions();
            perms.set_mode(mode & 0o777);
            tokio::fs::set_permissions(&native_path, perms)
                .await
                .map_err(VfsError::Io)?;
        }

        Ok(())
    }

    async fn write_file(
        &self,
        path: &Path,
        offset: u64,
        mut data: Pin<Box<dyn AsyncRead + Send + Sync + 'static>>,
    ) -> VfsResult<()> {
        let native_path = self.to_native_path(path);

        let meta = self.symlink_metadata_checked(&native_path).await?;
        if !meta.is_file() {
            return Err(VfsError::NotAFile(path.to_path_buf()));
        }

        let mut file = OpenOptions::new()
            .write(true)
            .open(native_path)
            .await
            .map_err(VfsError::Io)?;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(VfsError::Io)?;
        tokio::io::copy(&mut data, &mut file)
            .await
            .map_err(VfsError::Io)?;

        Ok(())
    }

    async fn remove(&self, path: &Path) -> VfsResult<()> {
        let native_path = self.to_native_path(path);

        let _ = self.symlink_metadata_checked(&native_path).await?;

        let metadata = self.symlink_metadata_checked(&native_path).await?;

        if metadata.is_dir() {
            let mut dir = tokio::fs::read_dir(&native_path)
                .await
                .map_err(VfsError::Io)?;
            if dir.next_entry().await.map_err(VfsError::Io)?.is_some() {
                return Err(VfsError::NotEmpty(path.to_path_buf()));
            }

            tokio::fs::remove_dir(&native_path)
                .await
                .map_err(VfsError::Io)
        } else {
            tokio::fs::remove_file(&native_path)
                .await
                .map_err(VfsError::Io)
        }
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> VfsResult<()> {
        let native_old_path = self.to_native_path(old_path);
        let native_new_path = self.to_native_path(new_path);

        let _ = self.symlink_metadata_checked(&native_old_path).await?;

        if self
            .symlink_metadata_option(&native_new_path)
            .await?
            .is_some()
        {
            return Err(VfsError::AlreadyExists(new_path.to_path_buf()));
        }

        if let Some(parent) = native_new_path.parent() {
            match self.symlink_metadata_checked(parent).await {
                Ok(_) => {}
                Err(VfsError::NotFound(_)) => {
                    return Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf()))
                }
                Err(e) => return Err(e),
            }
        }

        tokio::fs::rename(native_old_path, native_new_path)
            .await
            .map_err(VfsError::Io)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::{Group, Other, User};

    use super::*;

    #[tokio::test]
    async fn test_exists() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Test non-existent path
        assert!(!fs.exists(Path::new("nonexistent")).await.unwrap());

        // Create and test file
        fs.create_file(Path::new("test.txt"), false).await.unwrap();
        assert!(fs.exists(Path::new("test.txt")).await.unwrap());

        // Create and test directory
        fs.create_directory(Path::new("testdir")).await.unwrap();
        assert!(fs.exists(Path::new("testdir")).await.unwrap());
    }

    #[tokio::test]
    async fn test_create_file() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Test creating new file
        fs.create_file(Path::new("test.txt"), false).await.unwrap();
        assert!(fs.exists(Path::new("test.txt")).await.unwrap());

        // Test exists_ok = false
        let err = fs
            .create_file(Path::new("test.txt"), false)
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::AlreadyExists(_)));

        // Test exists_ok = true
        fs.create_file(Path::new("test.txt"), true).await.unwrap();

        // Test creating file in non-existent directory
        let err = fs
            .create_file(Path::new("nonexistent/test.txt"), false)
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::ParentDirectoryNotFound(_)));
    }

    #[tokio::test]
    async fn test_create_directory() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Test creating new directory
        fs.create_directory(Path::new("testdir")).await.unwrap();
        assert!(fs.exists(Path::new("testdir")).await.unwrap());

        // Test creating existing directory
        let err = fs.create_directory(Path::new("testdir")).await.unwrap_err();
        assert!(matches!(err, VfsError::AlreadyExists(_)));

        // Test creating nested directory without parent
        let err = fs
            .create_directory(Path::new("nonexistent/nested"))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::ParentDirectoryNotFound(_)));

        // Test creating nested directory with parent
        fs.create_directory(Path::new("parent")).await.unwrap();
        fs.create_directory(Path::new("parent/nested"))
            .await
            .unwrap();
        assert!(fs.exists(Path::new("parent/nested")).await.unwrap());
    }

    #[tokio::test]
    async fn test_create_symlink() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create target file
        fs.create_file(Path::new("target.txt"), false)
            .await
            .unwrap();

        // Test creating symlink
        fs.create_symlink(Path::new("link"), Path::new("target.txt"))
            .await
            .unwrap();
        assert!(fs.exists(Path::new("link")).await.unwrap());

        // Test creating existing symlink
        let err = fs
            .create_symlink(Path::new("link"), Path::new("target.txt"))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::AlreadyExists(_)));

        // Test creating symlink in non-existent directory
        let err = fs
            .create_symlink(Path::new("nonexistent/link"), Path::new("target.txt"))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::ParentDirectoryNotFound(_)));
    }

    #[tokio::test]
    async fn test_read_write_file() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create test file
        fs.create_file(Path::new("test.txt"), false).await.unwrap();

        // Write data
        let write_data = b"Hello, World!".to_vec();
        let reader = std::io::Cursor::new(write_data.clone());
        fs.write_file(Path::new("test.txt"), 0, Box::pin(reader))
            .await
            .unwrap();

        // Read data
        let mut reader = fs
            .read_file(Path::new("test.txt"), 0, write_data.len() as u64)
            .await
            .unwrap();
        let mut read_data = Vec::new();
        tokio::io::copy(&mut reader, &mut read_data).await.unwrap();
        assert_eq!(read_data, write_data);

        // Test reading with offset
        let mut reader = fs.read_file(Path::new("test.txt"), 7, 5).await.unwrap();
        let mut read_data = Vec::new();
        tokio::io::copy(&mut reader, &mut read_data).await.unwrap();
        assert_eq!(read_data, b"World");

        // Test reading non-existent file
        match fs.read_file(Path::new("nonexistent"), 0, 1).await {
            Err(VfsError::NotFound(_)) => {}
            _ => panic!("Expected NotFound error"),
        }

        // Test reading directory as file
        fs.create_directory(Path::new("testdir")).await.unwrap();
        match fs.read_file(Path::new("testdir"), 0, 1).await {
            Err(VfsError::NotAFile(_)) => {}
            _ => panic!("Expected NotAFile error"),
        }
    }

    #[tokio::test]
    async fn test_read_directory() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create test directory structure
        fs.create_directory(Path::new("testdir")).await.unwrap();
        fs.create_file(Path::new("testdir/file1.txt"), false)
            .await
            .unwrap();
        fs.create_file(Path::new("testdir/file2.txt"), false)
            .await
            .unwrap();
        fs.create_directory(Path::new("testdir/subdir"))
            .await
            .unwrap();

        // Read directory
        let entries: Vec<_> = fs
            .read_directory(Path::new("testdir"))
            .await
            .unwrap()
            .collect();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&PathSegment::try_from("file1.txt").unwrap()));
        assert!(entries.contains(&PathSegment::try_from("file2.txt").unwrap()));
        assert!(entries.contains(&PathSegment::try_from("subdir").unwrap()));

        // Test reading non-existent directory
        match fs.read_directory(Path::new("nonexistent")).await {
            Err(VfsError::NotFound(_)) => {}
            _ => panic!("Expected NotFound error"),
        }

        // Test reading file as directory
        match fs.read_directory(Path::new("testdir/file1.txt")).await {
            Err(VfsError::NotADirectory(_)) => {}
            _ => panic!("Expected NotADirectory error"),
        }
    }

    #[tokio::test]
    async fn test_read_symlink() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create target and symlink
        fs.create_file(Path::new("target.txt"), false)
            .await
            .unwrap();
        fs.create_symlink(Path::new("link"), Path::new("target.txt"))
            .await
            .unwrap();

        // Read symlink
        let target = fs.read_symlink(Path::new("link")).await.unwrap();
        assert_eq!(target, PathBuf::from("target.txt"));

        // Test reading non-existent symlink
        let err = fs.read_symlink(Path::new("nonexistent")).await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));

        // Test reading non-symlink
        let err = fs.read_symlink(Path::new("target.txt")).await.unwrap_err();
        assert!(matches!(err, VfsError::NotASymlink(_)));
    }

    #[tokio::test]
    async fn test_get_set_metadata() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create test file
        fs.create_file(Path::new("test.txt"), false).await.unwrap();

        // Get metadata
        let metadata = fs.get_metadata(Path::new("test.txt")).await.unwrap();
        assert_eq!(metadata.get_size(), 0);
        #[cfg(unix)]
        assert_eq!(metadata.get_mode().get_type(), Some(ModeType::File));

        // Write data and check size
        let data = b"Hello, World!".to_vec();
        let reader = std::io::Cursor::new(data.clone());
        fs.write_file(Path::new("test.txt"), 0, Box::pin(reader))
            .await
            .unwrap();
        let metadata = fs.get_metadata(Path::new("test.txt")).await.unwrap();
        assert_eq!(metadata.get_size(), data.len() as u64);

        // Test getting metadata for non-existent file
        let err = fs.get_metadata(Path::new("nonexistent")).await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));

        // Test setting metadata
        #[cfg(unix)]
        {
            let mut new_metadata = Metadata::new(ModeType::File);
            new_metadata.set_permissions(User::RWX | Group::RX | Other::R);
            fs.set_metadata(Path::new("test.txt"), new_metadata)
                .await
                .unwrap();

            let metadata = fs.get_metadata(Path::new("test.txt")).await.unwrap();
            assert_eq!(metadata.get_permissions(), User::RWX | Group::RX | Other::R);
        }
    }

    #[tokio::test]
    async fn test_remove() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create test files and directories
        fs.create_file(Path::new("test.txt"), false).await.unwrap();
        fs.create_directory(Path::new("empty_dir")).await.unwrap();
        fs.create_directory(Path::new("nonempty_dir"))
            .await
            .unwrap();
        fs.create_file(Path::new("nonempty_dir/file.txt"), false)
            .await
            .unwrap();

        // Remove file
        fs.remove(Path::new("test.txt")).await.unwrap();
        assert!(!fs.exists(Path::new("test.txt")).await.unwrap());

        // Remove empty directory
        fs.remove(Path::new("empty_dir")).await.unwrap();
        assert!(!fs.exists(Path::new("empty_dir")).await.unwrap());

        // Try to remove non-empty directory
        let err = fs.remove(Path::new("nonempty_dir")).await.unwrap_err();
        assert!(matches!(err, VfsError::NotEmpty(_)));

        // Remove file from directory then remove directory
        fs.remove(Path::new("nonempty_dir/file.txt")).await.unwrap();
        fs.remove(Path::new("nonempty_dir")).await.unwrap();
        assert!(!fs.exists(Path::new("nonempty_dir")).await.unwrap());

        // Try to remove non-existent path
        let err = fs.remove(Path::new("nonexistent")).await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_rename() {
        let (_temp_dir, fs) = helper::setup_fs().await;

        // Create test file
        fs.create_file(Path::new("old.txt"), false).await.unwrap();

        // Test simple rename
        fs.rename(Path::new("old.txt"), Path::new("new.txt"))
            .await
            .unwrap();
        assert!(!fs.exists(Path::new("old.txt")).await.unwrap());
        assert!(fs.exists(Path::new("new.txt")).await.unwrap());

        // Test rename to existing path
        fs.create_file(Path::new("existing.txt"), false)
            .await
            .unwrap();
        let err = fs
            .rename(Path::new("new.txt"), Path::new("existing.txt"))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::AlreadyExists(_)));

        // Test rename non-existent file
        let err = fs
            .rename(Path::new("nonexistent"), Path::new("target.txt"))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));

        // Test rename to path with non-existent parent
        let err = fs
            .rename(Path::new("new.txt"), Path::new("nonexistent/target.txt"))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::ParentDirectoryNotFound(_)));
    }
}

#[cfg(test)]
mod helper {
    use tempfile::TempDir;

    use super::*;

    /// Helper function to create a temporary directory and NativeFileSystem instance
    pub(super) async fn setup_fs() -> (TempDir, NativeFileSystem) {
        let temp_dir = TempDir::new().unwrap();
        let fs = NativeFileSystem::new(temp_dir.path().to_path_buf());
        (temp_dir, fs)
    }
}

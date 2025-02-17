use std::{
    error::Error,
    fmt::{self, Display},
    io,
    path::PathBuf,
};

use thiserror::Error;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The result of a file system operation.
pub type VfsResult<T> = Result<T, VfsError>;

/// An error that occurred during a file system operation.
#[derive(pretty_error_debug::Debug, Error)]
pub enum VfsError {
    /// The parent directory does not exist
    #[error("parent directory does not exist: {0}")]
    ParentDirectoryNotFound(PathBuf),

    /// The path already exists
    #[error("path already exists: {0}")]
    AlreadyExists(PathBuf),

    /// The path does not exist
    #[error("path does not exist: {0}")]
    NotFound(PathBuf),

    /// The path is not a directory
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// The path is not a file
    #[error("path is not a file: {0}")]
    NotAFile(PathBuf),

    /// The path is not a symlink
    #[error("path is not a symlink: {0}")]
    NotASymlink(PathBuf),

    /// The directory is not empty
    #[error("directory is not empty: {0}")]
    NotEmpty(PathBuf),

    /// Invalid offset for read/write operation
    #[error("invalid offset {offset} for path: {path}")]
    InvalidOffset {
        /// The path of the file
        path: PathBuf,

        /// The offset that is invalid
        offset: u64,
    },

    /// Insufficient permissions to perform the operation
    #[error("insufficient permissions for operation on: {0}")]
    PermissionDenied(PathBuf),

    /// The filesystem is read-only
    #[error("filesystem is read-only")]
    ReadOnlyFilesystem,

    /// Invalid symlink target
    #[error("invalid symlink target: {0}")]
    InvalidSymlinkTarget(PathBuf),

    /// Empty path segment
    #[error("empty path segment")]
    EmptyPathSegment,

    /// Invalid path component (e.g. ".", "..", "/")
    #[error("invalid path component: {0}")]
    InvalidPathComponent(String),

    /// IO error during filesystem operation
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// Overlay filesystem requires at least one layer
    #[error("overlay filesystem requires at least one layer")]
    OverlayFileSystemRequiresAtLeastOneLayer,

    /// Custom error.
    #[error(transparent)]
    Custom(#[from] AnyError),
}

/// An error that can represent any error.
#[derive(Debug)]
pub struct AnyError {
    error: anyhow::Error,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl VfsError {
    /// Creates a new `Err` result.
    pub fn custom(error: impl Into<anyhow::Error>) -> VfsError {
        VfsError::Custom(AnyError {
            error: error.into(),
        })
    }
}

impl AnyError {
    /// Downcasts the error to a `T`.
    pub fn downcast<T>(&self) -> Option<&T>
    where
        T: Display + fmt::Debug + Send + Sync + 'static,
    {
        self.error.downcast_ref::<T>()
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Creates an `Ok` `VfsResult`.
#[allow(non_snake_case)]
pub fn Ok<T>(value: T) -> VfsResult<T> {
    Result::Ok(value)
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl PartialEq for AnyError {
    fn eq(&self, other: &Self) -> bool {
        self.error.to_string() == other.error.to_string()
    }
}

impl Display for AnyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl Error for AnyError {}

impl From<VfsError> for nfsserve::nfs::nfsstat3 {
    fn from(error: VfsError) -> Self {
        use nfsserve::nfs::nfsstat3;
        match error {
            VfsError::ParentDirectoryNotFound(_) => nfsstat3::NFS3ERR_NOENT,
            VfsError::AlreadyExists(_) => nfsstat3::NFS3ERR_EXIST,
            VfsError::NotFound(_) => nfsstat3::NFS3ERR_NOENT,
            VfsError::NotADirectory(_) => nfsstat3::NFS3ERR_NOTDIR,
            VfsError::NotAFile(_) => nfsstat3::NFS3ERR_INVAL,
            VfsError::NotASymlink(_) => nfsstat3::NFS3ERR_INVAL,
            VfsError::NotEmpty(_) => nfsstat3::NFS3ERR_NOTEMPTY,
            VfsError::InvalidOffset { .. } => nfsstat3::NFS3ERR_INVAL,
            VfsError::PermissionDenied(_) => nfsstat3::NFS3ERR_PERM,
            VfsError::ReadOnlyFilesystem => nfsstat3::NFS3ERR_ROFS,
            VfsError::InvalidSymlinkTarget(_) => nfsstat3::NFS3ERR_INVAL,
            VfsError::EmptyPathSegment => nfsstat3::NFS3ERR_INVAL,
            VfsError::InvalidPathComponent(_) => nfsstat3::NFS3ERR_INVAL,
            VfsError::Io(_) => nfsstat3::NFS3ERR_IO,
            VfsError::OverlayFileSystemRequiresAtLeastOneLayer => nfsstat3::NFS3ERR_INVAL,
            VfsError::Custom(_) => nfsstat3::NFS3ERR_IO,
        }
    }
}

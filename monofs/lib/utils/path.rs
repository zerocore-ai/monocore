//! Path utilities.

use std::path::PathBuf;

use typed_path::Utf8UnixPath;

use crate::{config::DEFAULT_MFSRUN_BIN_PATH, filesystem::Utf8UnixPathSegment, FsError, FsResult};

use super::MFSRUN_BIN_PATH_ENV_VAR;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The default suffix of the directory where the actual filesystem data is stored
pub const MFS_DIR_SUFFIX: &str = "mfs";

/// The directory where project logs are stored
pub const LOG_SUBDIR: &str = "log";

/// The directory where the filesystem's blocks are stored
pub const BLOCKS_SUBDIR: &str = "blocks";

/// The filename of the database that stores the filesystem's metadata
pub const FS_DB_FILENAME: &str = "fs.db";

/// The name of the symlink that links to the actual filesystem data
pub const MFS_LINK_FILENAME: &str = ".mfs_link";

/// The prefix for mfsrun log files
pub const MFSRUN_LOG_PREFIX: &str = "mfsrun";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Split the last component from a path.
pub fn split_last(path: &Utf8UnixPath) -> FsResult<(Option<&Utf8UnixPath>, Utf8UnixPathSegment)> {
    // Root path are not allowed
    if path.has_root() {
        return Err(FsError::PathHasRoot(path.to_string()));
    }

    // Empty paths are not allowed
    if path.as_str().is_empty() {
        return Err(FsError::PathIsEmpty);
    }

    let filename = path
        .file_name()
        .ok_or_else(|| FsError::InvalidPathComponent(path.to_string()))?
        .parse()?;

    let parent = path
        .parent()
        .and_then(|p| (!p.as_str().is_empty()).then_some(p));

    Ok((parent, filename))
}

/// Resolves the path to the mfsrun binary, checking both environment variable and default locations.
///
/// First checks the environment variable specified by MFSRUN_BIN_PATH_ENV_VAR.
/// If that's not set, falls back to DEFAULT_MFSRUN_BIN_PATH.
/// Returns an error if the binary is not found at the resolved location.
pub fn resolve_mfsrun_bin_path() -> FsResult<PathBuf> {
    const MFSRUN_ENV_SOURCE: &str = "environment variable";
    const MFSRUN_DEFAULT_SOURCE: &str = "default path";

    let (path, source) = std::env::var(MFSRUN_BIN_PATH_ENV_VAR)
        .map(|p| (PathBuf::from(p), MFSRUN_ENV_SOURCE))
        .unwrap_or_else(|_| {
            (
                PathBuf::from(DEFAULT_MFSRUN_BIN_PATH),
                MFSRUN_DEFAULT_SOURCE,
            )
        });

    if !path.exists() {
        return Err(FsError::MfsrunBinaryNotFound {
            path: path.to_string_lossy().to_string(),
            src: source.to_string(),
        });
    }

    Ok(path)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use typed_path::Utf8UnixPathBuf;

    #[test]
    fn test_split_last() -> FsResult<()> {
        // Positive cases
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("foo/bar/baz"))?,
            (
                Some(Utf8UnixPath::new("foo/bar")),
                Utf8UnixPathSegment::from_str("baz")?
            )
        );
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("foo/bar"))?,
            (
                Some(Utf8UnixPath::new("foo")),
                Utf8UnixPathSegment::from_str("bar")?
            )
        );
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("baz"))?,
            (None, Utf8UnixPathSegment::from_str("baz")?)
        );

        // Path with multiple components
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("foo/bar/baz"))?,
            (
                Some(Utf8UnixPath::new("foo/bar")),
                Utf8UnixPathSegment::from_str("baz")?
            )
        );

        // Path with spaces
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("path with/spaces in/file name"))?,
            (
                Some(Utf8UnixPath::new("path with/spaces in")),
                Utf8UnixPathSegment::from_str("file name")?
            )
        );

        // Unicode characters
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("路径/文件"))?,
            (
                Some(Utf8UnixPath::new("路径")),
                Utf8UnixPathSegment::from_str("文件")?
            )
        );

        // Path ending with slash
        assert_eq!(
            split_last(&Utf8UnixPathBuf::from("foo/bar/"))?,
            (
                Some(Utf8UnixPath::new("foo")),
                Utf8UnixPathSegment::from_str("bar")?
            )
        );

        // Negative cases
        assert!(matches!(
            split_last(&Utf8UnixPathBuf::from("")),
            Err(FsError::PathIsEmpty)
        ));

        assert!(matches!(
            split_last(&Utf8UnixPathBuf::from("/")),
            Err(FsError::PathHasRoot(_))
        ));

        assert!(matches!(
            split_last(&Utf8UnixPathBuf::from("/foo")),
            Err(FsError::PathHasRoot(_))
        ));

        Ok(())
    }
}

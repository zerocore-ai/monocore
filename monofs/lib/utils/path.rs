//! Path utilities.

use typed_path::Utf8UnixPath;

use crate::{filesystem::Utf8UnixPathSegment, FsError, FsResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The default suffix of the directory where the actual filesystem data is stored
pub const MFS_DATA_SUFFIX: &str = ".mfs";

/// The filename of the database that stores the filesystem's metadata
pub const FS_DB_FILENAME: &str = "fs.db";

/// The name of the symlink that links to the actual filesystem data
pub const MFS_LINK_FILENAME: &str = ".mfs_link";

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

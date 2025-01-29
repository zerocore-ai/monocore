use std::path::{Path, PathBuf};
use tokio::{fs, net::TcpListener};

use crate::{utils::path::MFS_LINK_FILENAME, FsError, FsResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Maximum depth to search for MFS root
const MAX_MFS_ROOT_SEARCH_DEPTH: u32 = 10;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Find the MFS root directory by searching up the directory tree for the MFS link file.
///
/// This function starts from the given path and traverses up the directory hierarchy
/// looking for the MFS link file (`.mfs_link`). It will search up to [`MAX_MFS_ROOT_SEARCH_DEPTH`]
/// parent directories before giving up.
pub async fn find_mfs_root(start_path: impl AsRef<Path>) -> FsResult<PathBuf> {
    let start_path = start_path.as_ref();
    let canonical_start = fs::canonicalize(start_path).await?;
    let mut current = canonical_start.clone();
    let mut depth = 0;

    while depth < MAX_MFS_ROOT_SEARCH_DEPTH {
        let mfs_link = current.join(MFS_LINK_FILENAME);
        if fs::metadata(&mfs_link).await.is_ok() {
            return Ok(current);
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
            depth += 1;
        } else {
            // We've reached the root directory
            return Err(FsError::NoMfsRootFound(
                canonical_start.to_string_lossy().to_string(),
            ));
        }
    }

    Err(FsError::MaxMfsRootSearchDepthReached {
        max_depth: MAX_MFS_ROOT_SEARCH_DEPTH,
        path: canonical_start.to_string_lossy().to_string(),
    })
}

/// Find the next available port starting from the provided port number
pub async fn find_available_port(host: &str, start_port: u32) -> FsResult<u32> {
    const MAX_PORT_ATTEMPTS: u32 = 100;
    let end_port = start_port + MAX_PORT_ATTEMPTS - 1;

    for port in start_port..=end_port {
        match TcpListener::bind((host, port as u16)).await {
            Ok(_) => return Ok(port),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => return Err(FsError::IoError(e)),
        }
    }

    Err(FsError::NoAvailablePorts {
        host: host.to_string(),
        start: start_port,
        end: end_port,
    })
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;
    use tokio::test;

    #[test]
    async fn test_find_mfs_root_success() {
        let (temp, mut path) = helper::setup_test_dir(3).await;

        // Create .mfs_link in the middle directory
        path.pop(); // Go up one level
        let mfs_link = path.join(MFS_LINK_FILENAME);
        File::create(&mfs_link).unwrap();

        // Search from the deepest directory
        path.push("dir_2");
        let result = find_mfs_root(&path).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().file_name().unwrap(), "dir_1");

        temp.close().unwrap();
    }

    #[test]
    async fn test_find_mfs_root_not_found() {
        let (temp, path) = helper::setup_test_dir(2).await;

        let result = find_mfs_root(&path).await;

        assert!(matches!(result, Err(FsError::NoMfsRootFound(_))));

        temp.close().unwrap();
    }

    #[test]
    async fn test_find_mfs_root_max_depth_exceeded() {
        let (temp, path) = helper::setup_test_dir(MAX_MFS_ROOT_SEARCH_DEPTH as usize + 1).await;

        let result = find_mfs_root(&path).await;

        assert!(matches!(
            result,
            Err(FsError::MaxMfsRootSearchDepthReached {
                max_depth: MAX_MFS_ROOT_SEARCH_DEPTH,
                ..
            })
        ));

        temp.close().unwrap();
    }

    #[test]
    async fn test_find_mfs_root_from_nonexistent_path() {
        let result = find_mfs_root("/nonexistent/path").await;
        assert!(matches!(result, Err(FsError::IoError(_))));
    }

    #[test]
    async fn test_find_mfs_root_in_current_dir() {
        let temp = TempDir::new().unwrap();
        let mfs_link = temp.path().join(MFS_LINK_FILENAME);
        File::create(&mfs_link).unwrap();

        let result = find_mfs_root(temp.path()).await;

        assert!(result.is_ok());
        let found_path = result.unwrap();
        let temp_path = fs::canonicalize(temp.path()).await.unwrap();
        assert_eq!(found_path, temp_path);

        temp.close().unwrap();
    }
}

#[cfg(test)]
mod helper {
    use tempfile::TempDir;

    use super::*;

    /// Helper function to create a temporary directory structure for testing
    pub(super) async fn setup_test_dir(depth: usize) -> (TempDir, PathBuf) {
        let temp = TempDir::new().unwrap();
        let mut current = temp.path().to_path_buf();

        // Create nested directories
        for i in 0..depth {
            current.push(format!("dir_{}", i));
            fs::create_dir(&current).await.unwrap();
        }

        (temp, current)
    }
}

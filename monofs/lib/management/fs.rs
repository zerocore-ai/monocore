use crate::{utils::path::MFS_DATA_SUFFIX, FsResult};
use std::path::PathBuf;
use tokio::fs;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initialize a new monofs filesystem at the specified path
pub async fn init_fs(system_path: Option<PathBuf>) -> FsResult<()> {
    // Default to current directory if no path specified
    let system_path = system_path.unwrap_or_else(|| PathBuf::from("."));

    // Create the mount directory if it doesn't exist
    fs::create_dir_all(&system_path).await?;

    // Get the canonicalized absolute path to handle . and .. cases
    let canonical_path = fs::canonicalize(&system_path).await?;

    // Create the .mfs directory adjacent to the mount point
    let mfs_path = format!("{}{}", canonical_path.display(), MFS_DATA_SUFFIX);
    fs::create_dir_all(&mfs_path).await?;

    Ok(())
}

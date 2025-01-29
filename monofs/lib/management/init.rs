use crate::utils::path::{BLOCKS_SUBDIR, FS_DB_FILENAME, LOG_SUBDIR, MFS_LINK_FILENAME};
use crate::{
    config::{DEFAULT_HOST, DEFAULT_NFS_PORT},
    utils::path::{self, MFS_DIR_SUFFIX},
    FsResult,
};
use std::path::PathBuf;
use tokio::{fs, process::Command};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Initialize a new monofs filesystem at the specified path and mount it
///
/// ## Arguments
/// * `mount_dir` - The path where the filesystem will be initialized and mounted. If None, uses current directory
///
/// ## Returns
/// The port number that was successfully used for mounting
///
/// ## Example
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// init_fs(Some("mfstest".into())).await?;
/// # Ok(())
/// # }
/// ```
pub async fn init_fs(mount_dir: Option<PathBuf>) -> FsResult<u32> {
    // Default to current directory if no path specified
    let mount_dir = mount_dir.unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&mount_dir).await?;

    // Ensure the mount directory is absolute
    let mount_dir = fs::canonicalize(&mount_dir).await?;
    tracing::info!("Mount point available at {}", mount_dir.display());

    // Create the .mfs directory adjacent to the mount point
    let mfs_data_dir = PathBuf::from(format!("{}.{}", mount_dir.display(), MFS_DIR_SUFFIX));
    fs::create_dir_all(&mfs_data_dir).await?;
    tracing::info!(".mfs directory available at {}", mfs_data_dir.display());

    // Find an available port
    let port = super::find_available_port(DEFAULT_HOST, DEFAULT_NFS_PORT)?;
    tracing::info!("Found available port: {}", port);

    // Create required directories
    let log_dir = mfs_data_dir.join(LOG_SUBDIR);
    fs::create_dir_all(&log_dir).await?;
    tracing::info!("Log directory available at {}", log_dir.display());

    // Create the fs database file
    let db_path = mfs_data_dir.join(FS_DB_FILENAME);
    if !db_path.exists() {
        fs::File::create(&db_path).await?;
        tracing::info!("Created fs database at {}", db_path.display());
    }

    // Create the blocks directory
    let blocks_dir = mfs_data_dir.join(BLOCKS_SUBDIR);
    fs::create_dir_all(&blocks_dir).await?;
    tracing::info!("Blocks directory available at {}", blocks_dir.display());

    // Start the supervisor process
    let child_name = mount_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .expect("Failed to get file name for mount point");

    let mfsrun_path = path::resolve_mfsrun_bin_path()?;

    tracing::info!("Mounting the filesystem...");
    let status = Command::new(mfsrun_path)
        .arg("supervisor")
        .arg("--log-dir")
        .arg(&log_dir)
        .arg("--child-name")
        .arg(child_name)
        .arg("--host")
        .arg(DEFAULT_HOST)
        .arg("--port")
        .arg(port.to_string())
        .arg("--store-dir")
        .arg(&blocks_dir)
        .arg("--db-path")
        .arg(&db_path)
        .spawn()?;

    tracing::info!(
        "Started supervisor process with PID: {}",
        status.id().unwrap_or(0)
    );

    // Mount the filesystem
    super::mount_fs(&mount_dir, DEFAULT_HOST, port).await?;
    tracing::info!("Mounted filesystem at {}", mount_dir.display());

    // Create symbolic link to mfs_data_dir in mount directory
    let link_path = mount_dir.join(MFS_LINK_FILENAME);
    if !link_path.exists() {
        fs::symlink(&mfs_data_dir, &link_path).await?;
        tracing::info!("Created symbolic link at {}", link_path.display());
    }

    Ok(port)
}

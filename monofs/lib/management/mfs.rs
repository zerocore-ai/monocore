use crate::management::find;
use crate::utils::path::{BLOCKS_SUBDIR, FS_DB_FILENAME, LOG_SUBDIR, MFS_LINK_FILENAME};
use crate::FsError;
use crate::{
    config::{DEFAULT_HOST, DEFAULT_NFS_PORT},
    utils::path::{self, MFS_DIR_SUFFIX},
    FsResult,
};
use std::path::{Path, PathBuf};
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
/// init_mfs(Some("mfstest".into())).await?;
/// # Ok(())
/// # }
/// ```
pub async fn init_mfs(mount_dir: Option<PathBuf>) -> FsResult<u32> {
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
    let port = super::find_available_port(DEFAULT_HOST, DEFAULT_NFS_PORT).await?;
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
    mount_fs(&mount_dir, DEFAULT_HOST, port).await?;
    tracing::info!("Mounted filesystem at {}", mount_dir.display());

    // Create symbolic link to mfs_data_dir in mount directory
    let link_path = mount_dir.join(MFS_LINK_FILENAME);
    if !link_path.exists() {
        fs::symlink(&mfs_data_dir, &link_path).await?;
        tracing::info!("Created symbolic link at {}", link_path.display());
    }

    Ok(port)
}

/// Detach a monofs filesystem by finding its root and unmounting it
///
/// ## Arguments
/// * `mount_dir` - Optional path to start searching from. If None, uses current directory
/// * `force` - Whether to force unmount even if the filesystem is busy
///
/// ## Example
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// // Detach from current directory
/// detach_mfs(None, false).await?;
///
/// // Detach from specific directory with force option
/// detach_mfs(Some("mfstest".into()), true).await?;
/// # Ok(())
/// # }
/// ```
pub async fn detach_mfs(mount_dir: Option<PathBuf>, force: bool) -> FsResult<()> {
    // Default to current directory if no path specified
    let start_path = mount_dir.unwrap_or_else(|| PathBuf::from("."));

    // Find the MFS root directory
    let mfs_root = find::find_mfs_root(&start_path).await?;
    tracing::info!("Found MFS root at {}", mfs_root.display());

    // Unmount the filesystem
    unmount_fs(&mfs_root, force).await
}

/// Unmount a filesystem at the specified mount point
async fn unmount_fs(mount_dir: impl AsRef<Path>, force: bool) -> FsResult<()> {
    let mount_dir = mount_dir.as_ref();

    // Check if mount point exists
    if !mount_dir.exists() {
        return Err(FsError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Mount point does not exist: {}", mount_dir.display()),
        )));
    }

    tracing::info!("Unmounting filesystem at {}", mount_dir.display());

    // Construct the unmount command
    let mut cmd = Command::new("umount");
    if force {
        cmd.arg("-f");
    }
    cmd.arg(mount_dir);

    let status = cmd.status().await?;

    if !status.success() {
        return Err(FsError::UnmountFailed(format!(
            "Unmount command exited with status: {}",
            status
        )));
    }

    tracing::info!(
        "Successfully unmounted filesystem at {}",
        mount_dir.display()
    );
    Ok(())
}

/// Mount a remote NFS filesystem at the specified mount point
async fn mount_fs(mount_dir: impl AsRef<Path>, host: &str, port: u32) -> FsResult<()> {
    let mount_dir = mount_dir.as_ref();

    // Create mount point if it doesn't exist
    fs::create_dir_all(&mount_dir).await?;
    tracing::info!("Mount point available at {}", mount_dir.display());

    // Check if mount point is empty
    let mut entries = fs::read_dir(&mount_dir).await?;
    if entries.next_entry().await?.is_some() {
        return Err(FsError::MountPointNotEmpty(
            mount_dir.to_string_lossy().to_string(),
        ));
    }
    tracing::info!("Mounting NFS share at {}", mount_dir.display());

    // Construct the mount command
    // Using standard NFS mount options:
    // - nolocks: disable NFS file locking
    // - vers=3: use NFSv3
    // - tcp: use TCP transport
    // - soft: return errors rather than hang on timeouts
    // - mountport=port: use same port for mount protocol
    let source = format!("{}:/", host);
    let status = Command::new("mount")
        .arg("-t")
        .arg("nfs")
        .arg("-o")
        .arg(format!(
            "nolocks,vers=3,tcp,port={port},mountport={port},soft",
            port = port
        ))
        .arg(source)
        .arg(&mount_dir)
        .status()
        .await?;

    if !status.success() {
        return Err(FsError::MountFailed(format!(
            "Mount command exited with status: {}",
            status
        )));
    }

    tracing::info!("Successfully mounted NFS share at {}", mount_dir.display());
    Ok(())
}

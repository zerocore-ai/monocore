use crate::{FsError, FsResult};
use std::{net::TcpListener, path::Path};
use tokio::{fs, process::Command};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Mount a remote NFS filesystem at the specified mount point
///
/// ## Arguments
/// * `mount_dir` - The local directory to use as the mount point. Must exist and be empty
/// * `host` - The NFS server host address (e.g. "127.0.0.1")
/// * `port` - The port the NFS server is listening on
///
/// ## Errors
/// Returns an error if:
/// - The mount point does not exist or cannot be created (`IoError`)
/// - The mount point is not empty (`MountPointNotEmpty`)
/// - The mount operation fails or lacks necessary permissions (`MountFailed`)
/// - The NFS server is unreachable (`MountFailed`)
///
/// ## Example
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// mount_fs("mfstest", "127.0.0.1", 2049).await?;
/// # Ok(())
/// # }
/// ```
pub async fn mount_fs(mount_dir: impl AsRef<Path>, host: &str, port: u32) -> FsResult<()> {
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

/// Find the next available port starting from the provided port number
pub fn find_available_port(host: &str, start_port: u32) -> FsResult<u32> {
    const MAX_PORT_ATTEMPTS: u32 = 100;
    let end_port = start_port + MAX_PORT_ATTEMPTS - 1;

    for port in start_port..=end_port {
        match TcpListener::bind((host, port as u16)) {
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

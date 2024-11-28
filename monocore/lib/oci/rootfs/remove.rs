use std::path::Path;

use tokio::fs;

use crate::{utils::MERGED_SUBDIR, MonocoreResult};

use super::PermissionGuard;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Unmounts an overlayfs mount point if it exists.
/// Does nothing on non-Linux platforms or when overlayfs is not enabled.
///
/// # Arguments
/// * `dest_dir` - Directory containing the merged layers
///
/// # Platform-specific behavior
/// - Linux with overlayfs: Unmounts the overlayfs mount at dest_dir/merged
/// - Other platforms: No-op
///
/// # Errors
/// Returns error if unmounting fails on Linux with overlayfs
pub async fn unmount(#[allow(unused_variables)] dest_dir: impl AsRef<Path>) -> MonocoreResult<()> {
    // Only try to unmount on Linux with overlayfs feature
    #[cfg(all(target_os = "linux", feature = "overlayfs"))]
    {
        use crate::MonocoreError;
        use std::process::Command;

        let merged_dir = dest_dir.as_ref().join(MERGED_SUBDIR);

        // Check if directory is a mount point
        let output = Command::new("mountpoint")
            .arg("-q") // Quiet mode
            .arg(&merged_dir)
            .status();

        if output.map_or(false, |status| status.success()) {
            tracing::info!("Unmounting overlayfs at {}", merged_dir.display());

            // First try a normal unmount
            let status = Command::new("umount").arg(&merged_dir).status()?;

            if !status.success() {
                tracing::warn!("Failed to unmount overlayfs normally, trying force unmount");

                // Try force unmount if normal unmount fails
                let force_status = Command::new("umount")
                    .arg("-f") // Force unmount
                    .arg(&merged_dir)
                    .status()?;

                if !force_status.success() {
                    tracing::warn!("Force unmount failed, trying lazy unmount");
                    // If force unmount also fails, try lazy unmount as last resort
                    let lazy_status = Command::new("umount")
                        .arg("-l") // Lazy unmount
                        .arg(&merged_dir)
                        .status()?;

                    if !lazy_status.success() {
                        tracing::error!("All unmount attempts failed for {}", merged_dir.display());
                        return Err(MonocoreError::LayerHandling {
                            source: std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Failed to unmount overlayfs",
                            ),
                            layer: merged_dir.display().to_string(),
                        });
                    }
                }
            }

            // Give the system a moment to complete the unmount
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    Ok(())
}

/// Removes an overlayfs mount point and its associated directories.
///
/// This function:
/// 1. Unmounts the overlayfs mount point if it exists (Linux only)
/// 2. Removes the following directories under dest_dir:
///    - merged/ - The merged filesystem view
///    - work/   - Overlayfs work directory
///    - upper/  - Overlayfs upper directory
///
/// # Platform-specific behavior
/// - Linux: Uses `rm` command with appropriate flags for efficient removal
/// - Other platforms: Uses recursive directory removal with permission fixes
///
/// # Arguments
/// * `dest_dir` - Base directory containing the overlayfs mount
///
/// # Errors
/// Returns error if:
/// - Failed to unmount (Linux only)
/// - Failed to fix permissions for cleanup
/// - Failed to remove directories
/// - On Linux: Failed to execute rm command
pub async fn remove(dest_dir: impl AsRef<Path>) -> MonocoreResult<()> {
    let dest_dir = dest_dir.as_ref();
    unmount(dest_dir).await?;
    remove_internal(dest_dir).await
}

async fn remove_internal(dest_dir: impl AsRef<Path>) -> MonocoreResult<()> {
    let merged_dir = dest_dir.as_ref().join(MERGED_SUBDIR);
    if merged_dir.exists() {
        let mut perm_guard = PermissionGuard::new();
        let mut dir_stack = vec![merged_dir.clone()];

        // Process directories in a stack-based manner
        while let Some(dir) = dir_stack.pop() {
            perm_guard.make_readable_writable(&dir).await?;

            // Read and queue subdirectories
            if let Ok(mut entries) = fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(file_type) = entry.file_type().await {
                        if file_type.is_dir() {
                            dir_stack.push(entry.path());
                        }
                    }
                }
            }
        }

        // Now remove the directory tree
        match fs::remove_dir_all(&merged_dir).await {
            Ok(_) => tracing::debug!("Successfully removed {}", merged_dir.display()),
            Err(e) => {
                tracing::error!("Failed to remove {}: {}", merged_dir.display(), e);
                return Err(e.into());
            }
        }
    }
    Ok(())
}

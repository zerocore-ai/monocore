use std::{
    collections::HashMap,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use getset::Getters;
use tokio::fs;

use crate::{utils, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Tracks and manages temporary permission changes for files and directories.
///
/// This guard keeps track of original file/directory permissions when they need to be temporarily
/// modified (e.g., to make them writable during operations). When the guard is dropped, it
/// automatically restores all original permissions in reverse order.
///
/// # Example
/// ```no_run
/// use monocore::{oci::rootfs::PermissionGuard, MonocoreResult};
///
/// #[tokio::main]
/// async fn main() -> MonocoreResult<()> {
///     let mut guard = PermissionGuard::new();
///
///     // Temporarily make a path writable
///     guard.make_writable("some/readonly/path").await?;
///
///     // Do some work that requires write permissions...
///
///     // Permissions are automatically restored when guard is dropped
///     Ok(())
/// }
/// ```
#[derive(Debug, Default, Getters)]
#[getset(get = "pub with_prefix")]
pub struct PermissionGuard {
    /// Maps paths to their original permissions
    original_modes: HashMap<PathBuf, u32>,

    /// Paths in order they were modified (for proper restoration)
    modified_paths: Vec<PathBuf>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PermissionGuard {
    /// Creates a new empty permission guard.
    ///
    /// The guard starts with no tracked permissions. As paths are made writable or readable,
    /// their original permissions will be saved and restored when the guard is dropped.
    ///
    /// # Example
    /// ```no_run
    /// use monocore::oci::rootfs::PermissionGuard;
    ///
    /// let mut guard = PermissionGuard::new();
    /// ```
    pub fn new() -> Self {
        Self {
            original_modes: HashMap::new(),
            modified_paths: Vec::new(),
        }
    }

    /// Makes a path temporarily writable while preserving its other permission bits.
    ///
    /// This method:
    /// 1. Saves the path's original permissions
    /// 2. Adds write and execute permissions while preserving other bits
    /// 3. Skips symlinks since their permissions can't be modified
    ///
    /// The original permissions will be restored when the guard is dropped.
    ///
    /// # Arguments
    /// * `path` - The path to make writable
    ///
    /// # Errors
    /// Returns error if:
    /// * Failed to read path metadata
    /// * Failed to set new permissions
    pub async fn make_writable(&mut self, path: impl AsRef<Path>) -> MonocoreResult<()> {
        let path = path.as_ref();
        // Skip if we've already processed this path or if it's a symlink
        if !self.original_modes.contains_key(path) {
            if let Ok(metadata) = fs::symlink_metadata(path).await {
                if metadata.file_type().is_symlink() {
                    tracing::debug!(
                        "Skipping permission modification for symlink: {}",
                        path.display()
                    );
                    return Ok(());
                }

                let mode = metadata.permissions().mode();
                self.original_modes.insert(path.to_path_buf(), mode);
                self.modified_paths.push(path.to_path_buf());

                // Make path writable and executable while preserving other bits
                let wx_mode = mode | 0o300;
                fs::set_permissions(path, std::fs::Permissions::from_mode(wx_mode)).await?;
                tracing::debug!(
                    "Made path writable: {}, mode: {} -> {} ({:#o} -> {:#o})",
                    path.display(),
                    utils::format_mode(mode),
                    utils::format_mode(wx_mode),
                    mode,
                    wx_mode
                );
            }
        }
        Ok(())
    }

    /// Makes a path temporarily readable and writable while preserving other permission bits.
    ///
    /// This method:
    /// 1. Saves the path's original permissions
    /// 2. Adds read, write and execute permissions while preserving other bits
    /// 3. Skips symlinks since their permissions can't be modified
    ///
    /// The original permissions will be restored when the guard is dropped.
    ///
    /// # Arguments
    /// * `path` - The path to make readable and writable
    ///
    /// # Errors
    /// Returns error if:
    /// * Failed to read path metadata
    /// * Failed to set new permissions
    pub async fn make_readable_writable(&mut self, path: impl AsRef<Path>) -> MonocoreResult<()> {
        let path = path.as_ref();
        if !self.original_modes.contains_key(path) {
            if let Ok(metadata) = fs::symlink_metadata(path).await {
                if metadata.file_type().is_symlink() {
                    tracing::debug!(
                        "Skipping permission modification for symlink: {}",
                        path.display()
                    );
                    return Ok(());
                }

                let mode = metadata.permissions().mode();
                self.original_modes.insert(path.to_path_buf(), mode);
                self.modified_paths.push(path.to_path_buf());

                // Make path readable, writable and executable while preserving other bits
                let rwx_mode = mode | 0o700;
                fs::set_permissions(path, std::fs::Permissions::from_mode(rwx_mode)).await?;
                tracing::debug!(
                    "Made path readable/writable: {}, mode: {} -> {} ({:#o} -> {:#o})",
                    path.display(),
                    utils::format_mode(mode),
                    utils::format_mode(rwx_mode),
                    mode,
                    rwx_mode
                );
            }
        }
        Ok(())
    }

    /// Restores original permissions for all modified paths in reverse order
    fn restore_all(&mut self) -> MonocoreResult<()> {
        while let Some(path) = self.modified_paths.pop() {
            if let Some(&original_mode) = self.original_modes.get(&path) {
                // Skip restoration if path no longer exists
                if !path.exists() {
                    tracing::debug!(
                        "Skipping permission restoration for deleted path: {}",
                        path.display()
                    );
                    continue;
                }

                // Skip symlinks since we can't set their permissions
                if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                    if metadata.file_type().is_symlink() {
                        tracing::debug!(
                            "Skipping permission restoration for symlink: {}",
                            path.display()
                        );
                        continue;
                    }
                }

                if let Err(e) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(original_mode))
                {
                    tracing::warn!(
                        "Failed to restore permissions for {}: {}",
                        path.display(),
                        e
                    );
                    return Err(e.into());
                } else {
                    tracing::debug!(
                        "Restored permissions for: {}, mode: {} ({:#o})",
                        path.display(),
                        utils::format_mode(original_mode),
                        original_mode
                    );
                }
            }
        }
        self.original_modes.clear();
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Drop for PermissionGuard {
    fn drop(&mut self) {
        if !self.modified_paths.is_empty() {
            // Don't propagate errors in drop, just log them
            if let Err(e) = self.restore_all() {
                tracing::debug!("Error during permission restoration in drop: {}", e);
            }
        }
    }
}

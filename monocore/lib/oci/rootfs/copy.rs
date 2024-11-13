#![cfg(not(target_os = "linux"))]
use std::path::Path;

use tokio::fs;

use crate::{oci::rootfs::PermissionGuard, MonocoreError, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const WHITEOUT_PREFIX: &str = ".wh.";
const WHITEOUT_OPAQUE: &str = ".wh..wh..opq";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Copies a directory tree while handling whiteouts and preserving permissions.
///
/// This function implements OCI-style layer copying with whiteout handling:
/// - Regular whiteouts (.wh.file) remove the corresponding file/directory
/// - Opaque whiteouts (.wh..wh..opq) hide all existing contents of a directory
///
/// File permissions are preserved using a PermissionGuard to handle restricted permissions
/// during the copy operation.
///
/// # Arguments
/// * `source_dir` - Source directory to copy from
/// * `dest_dir` - Destination directory to copy to
/// * `process_whiteouts` - Whether to process whiteout files (true for layer merging)
///
/// # Errors
/// Returns error if:
/// * Failed to read source directory
/// * Failed to create destination directory
/// * Failed to copy files
/// * Failed to set permissions
pub async fn copy(
    source_dir: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
    process_whiteouts: bool,
) -> MonocoreResult<()> {
    let source_dir = source_dir.as_ref();
    let dest_dir = dest_dir.as_ref();

    let mut stack = vec![source_dir.to_path_buf()];
    let mut perm_guard = PermissionGuard::new();

    while let Some(current_path) = stack.pop() {
        // Make current directory readable to list contents
        perm_guard.make_readable_writable(&current_path).await?;

        let mut entries =
            fs::read_dir(&current_path)
                .await
                .map_err(|e| MonocoreError::LayerHandling {
                    source: e,
                    layer: source_dir.display().to_string(),
                })?;

        let target_dir = dest_dir.join(current_path.strip_prefix(source_dir).unwrap());
        fs::create_dir_all(&target_dir).await?;

        while let Some(entry) =
            entries
                .next_entry()
                .await
                .map_err(|e| MonocoreError::LayerHandling {
                    source: e,
                    layer: source_dir.display().to_string(),
                })?
        {
            let path = entry.path();
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Handle whiteouts
            if process_whiteouts && file_name_str.starts_with(WHITEOUT_PREFIX) {
                if file_name_str == WHITEOUT_OPAQUE {
                    let target_dir = dest_dir.join(current_path.strip_prefix(source_dir).unwrap());

                    // Remove target directory if it exists
                    if target_dir.exists() {
                        fs::remove_dir_all(&target_dir).await?;
                    }
                    fs::create_dir_all(&target_dir).await?;

                    // Process remaining files in this directory
                    let mut entries = fs::read_dir(&current_path).await?;
                    while let Some(sibling) = entries.next_entry().await? {
                        let sibling_name = sibling.file_name();
                        let sibling_name_str = sibling_name.to_string_lossy();

                        // Skip the opaque whiteout file itself
                        if sibling_name_str == WHITEOUT_OPAQUE {
                            continue;
                        }

                        let sibling_path = sibling.path();
                        let relative_path = sibling_path.strip_prefix(source_dir).unwrap();
                        let target_path = dest_dir.join(relative_path);

                        handle_fs_entry(&sibling_path, &target_path, &perm_guard).await?;
                        if fs::symlink_metadata(&sibling_path)
                            .await?
                            .file_type()
                            .is_dir()
                        {
                            stack.push(sibling_path);
                        }
                    }
                    continue;
                } else {
                    let original_name = file_name_str.trim_start_matches(WHITEOUT_PREFIX);
                    let target_path = target_dir.join(original_name);

                    if target_path.exists() {
                        if target_path.is_dir() {
                            fs::remove_dir_all(&target_path).await?;
                        } else {
                            fs::remove_file(&target_path).await?;
                        }
                    }

                    continue;
                }
            }

            // Copy files
            let relative_path = path.strip_prefix(source_dir).unwrap();
            let target_path = dest_dir.join(relative_path);

            // Make source readable and target's parent writable
            perm_guard.make_readable_writable(&path).await?;
            if let Some(parent) = target_path.parent() {
                perm_guard.make_writable(parent).await?;
            }

            handle_fs_entry(&path, &target_path, &perm_guard).await?;
            if fs::symlink_metadata(&path).await?.file_type().is_dir() {
                stack.push(path);
            }
        }
    }

    Ok(())
}

/// Handles copying, creating directories, or creating symlinks from source to target path
#[cfg(not(target_os = "linux"))]
async fn handle_fs_entry(
    source_path: &Path,
    target_path: &Path,
    perm_guard: &PermissionGuard,
) -> MonocoreResult<()> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

    use nix::{sys::stat::Mode, unistd};

    use crate::utils;

    let metadata = fs::symlink_metadata(source_path).await?;
    let file_type = metadata.file_type();

    if file_type.is_dir() {
        tracing::debug!("Creating directory: {}", target_path.display());
        fs::create_dir_all(target_path).await?;
    } else if file_type.is_file() {
        tracing::debug!(
            "Copying file: {} -> {}",
            source_path.display(),
            target_path.display()
        );

        // Check if it's a hard link by comparing inode numbers
        let source_dev = metadata.dev();
        let source_ino = metadata.ino();

        if let Ok(target_metadata) = fs::symlink_metadata(target_path).await {
            if target_metadata.dev() == source_dev && target_metadata.ino() == source_ino {
                // Already linked, nothing to do
                return Ok(());
            }
        }

        fs::copy(source_path, target_path).await?;
    } else if file_type.is_symlink() {
        tracing::debug!(
            "Creating symlink: {} -> {}",
            target_path.display(),
            source_path.display()
        );
        let link_target = fs::read_link(source_path).await?;

        // Remove existing symlink or file if it exists
        if target_path.exists() {
            if target_path.is_dir() {
                fs::remove_dir_all(target_path).await?;
            } else {
                fs::remove_file(target_path).await?;
            }
        }

        // Create the symlink with the original target path
        fs::symlink(&link_target, target_path).await?;

        // Skip setting permissions for symlinks since they don't have their own permissions
        return Ok(());
    } else if file_type.is_fifo() {
        tracing::debug!("Creating FIFO: {}", target_path.display());
        if target_path.exists() {
            fs::remove_file(target_path).await?;
        }

        // Create FIFO with same permissions as source
        let mode = Mode::from_bits_truncate(metadata.mode() as u16 & 0o777);
        unistd::mkfifo(target_path, mode)?;
    }

    // Set the original permissions on the target
    let original_mode = perm_guard
        .get_original_modes()
        .get(source_path)
        .copied()
        .unwrap_or_else(|| metadata.permissions().mode());

    fs::set_permissions(target_path, std::fs::Permissions::from_mode(original_mode)).await?;
    tracing::debug!(
        "Applied original permissions to {}: {} ({:#o})",
        target_path.display(),
        utils::format_mode(original_mode),
        original_mode
    );

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::{
        os::unix::fs::{FileTypeExt, PermissionsExt},
        path::PathBuf,
    };

    use tempfile::tempdir;

    use crate::utils;

    use super::*;

    #[test_log::test(tokio::test)]
    /// Tests copying files with various permissions and special files.
    ///
    /// Test Structure:
    /// ```text
    /// source/                             dest/
    /// ├── test.txt       (rw-r--r--) ───→ ├── test.txt       (rw-r--r--)
    /// ├── readonly.txt   (r--r--r--) ───→ ├── readonly.txt   (r--r--r--)
    /// ├── writeonly.txt  (-w--w--w-) ───→ ├── writeonly.txt  (-w--w--w-)
    /// ├── test.fifo      (rw-r--r--) ───→ ├── test.fifo      (rw-r--r--)
    /// ├── link.txt     → test.txt ──────→ ├── link.txt     → test.txt
    /// ├── inner_link.txt → restricted/inner.txt
    /// │                                   ├── inner_link.txt → restricted/inner.txt
    /// │                                   │
    /// ├── restricted/    (--x------) ───→ ├── restricted/   (--x------)
    /// │   └── inner.txt  (r--------) ───→ │   └── inner.txt (r--------)
    /// │
    /// Contents:
    /// - test.txt:       "test content"
    /// - readonly.txt:   "readonly content"
    /// - writeonly.txt:  "writeonly content"
    /// - inner.txt:      "inner content"
    /// ```
    async fn test_oci_rootfs_copy_with_permissions() -> anyhow::Result<()> {
        // Create source and destination directories
        let temp = tempdir()?;
        let source_dir = temp.path().join("source");
        let dest_dir = temp.path().join("dest");

        // Create test fixtures
        helper::create_test_fixtures(&source_dir).await?;

        // Perform the copy
        copy(&source_dir, &dest_dir, true).await?;

        // Verify the copy
        let verify_perms = |path: &Path, expected_mode: u32| -> anyhow::Result<()> {
            let metadata = std::fs::metadata(path)?;
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(
                mode,
                expected_mode,
                "Permission mismatch for {}: expected {} ({:#o}), got {} ({:#o})",
                path.display(),
                utils::format_mode(expected_mode),
                expected_mode,
                utils::format_mode(mode),
                mode
            );
            Ok(())
        };

        // Verify regular file
        let dest_test_file = dest_dir.join("test.txt");
        assert!(dest_test_file.exists());
        verify_perms(&dest_test_file, 0o644)?;
        assert_eq!(fs::read_to_string(&dest_test_file).await?, "test content");

        // Verify readonly file
        let dest_readonly = dest_dir.join("readonly.txt");
        assert!(dest_readonly.exists());
        verify_perms(&dest_readonly, 0o444)?;
        assert_eq!(
            fs::read_to_string(&dest_readonly).await?,
            "readonly content"
        );

        // Verify writeonly file
        let dest_writeonly = dest_dir.join("writeonly.txt");
        assert!(dest_writeonly.exists());
        verify_perms(&dest_writeonly, 0o222)?;

        // Verify restricted directory and its contents
        let dest_restricted = dest_dir.join("restricted");
        assert!(dest_restricted.exists());
        verify_perms(&dest_restricted, 0o100)?;

        let dest_inner = dest_restricted.join("inner.txt");
        assert!(dest_inner.exists());
        verify_perms(&dest_inner, 0o400)?;
        assert_eq!(fs::read_to_string(&dest_inner).await?, "inner content");

        // Verify FIFO
        let dest_fifo = dest_dir.join("test.fifo");
        assert!(dest_fifo.exists());
        assert!(fs::metadata(&dest_fifo).await?.file_type().is_fifo());
        verify_perms(&dest_fifo, 0o644)?;

        // Verify symlinks
        let dest_link = dest_dir.join("link.txt");
        assert!(fs::symlink_metadata(&dest_link)
            .await?
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_link(&dest_link).await?, PathBuf::from("test.txt"));

        let dest_inner_link = dest_dir.join("inner_link.txt");
        assert!(fs::symlink_metadata(&dest_inner_link)
            .await?
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_link(&dest_inner_link).await?,
            PathBuf::from("restricted/inner.txt")
        );

        Ok(())
    }

    mod helper {
        use std::os::unix::fs::PermissionsExt;

        use nix::{sys::stat::Mode, unistd};

        use super::*;

        /// Creates test fixtures with various file types and permissions for testing copy functionality
        ///
        /// Creates the following structure:
        /// ```text
        /// source/
        /// ├── test.txt       (rw-r--r--) "test content"
        /// ├── readonly.txt   (r--r--r--) "readonly content"
        /// ├── writeonly.txt  (-w--w--w-) "writeonly content"
        /// ├── test.fifo      (rw-r--r--) [named pipe]
        /// ├── link.txt     → test.txt
        /// ├── inner_link.txt → restricted/inner.txt
        /// │
        /// └── restricted/   (--x------)
        ///     └── inner.txt (r--------) "inner content"
        /// ```
        pub(super) async fn create_test_fixtures(source_dir: &Path) -> anyhow::Result<()> {
            fs::create_dir(source_dir).await?;

            // Create test files and directories with various permissions
            let test_file = source_dir.join("test.txt");
            fs::write(&test_file, "test content").await?;
            fs::set_permissions(&test_file, std::fs::Permissions::from_mode(0o644)).await?;

            let readonly_file = source_dir.join("readonly.txt");
            fs::write(&readonly_file, "readonly content").await?;
            fs::set_permissions(&readonly_file, std::fs::Permissions::from_mode(0o444)).await?;

            let writeonly_file = source_dir.join("writeonly.txt");
            fs::write(&writeonly_file, "writeonly content").await?;
            fs::set_permissions(&writeonly_file, std::fs::Permissions::from_mode(0o222)).await?;

            let restricted_dir = source_dir.join("restricted");
            fs::create_dir(&restricted_dir).await?;

            // Create inner file first
            let inner_file = restricted_dir.join("inner.txt");
            fs::write(&inner_file, "inner content").await?;
            fs::set_permissions(&inner_file, std::fs::Permissions::from_mode(0o400)).await?; // r--------

            // Set directory permissions after creating inner file
            fs::set_permissions(&restricted_dir, std::fs::Permissions::from_mode(0o100)).await?; // --x------

            // Create a FIFO
            let fifo_path = source_dir.join("test.fifo");
            unistd::mkfifo(&fifo_path, Mode::from_bits_truncate(0o644))?;

            // Create symlinks
            std::os::unix::fs::symlink("test.txt", source_dir.join("link.txt"))?;
            std::os::unix::fs::symlink("restricted/inner.txt", source_dir.join("inner_link.txt"))?;

            Ok(())
        }
    }
}

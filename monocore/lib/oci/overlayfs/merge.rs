use std::{
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[cfg(target_os = "linux")]
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::{sys::stat::Mode, unistd};
use oci_spec::image::ImageManifest;
use tokio::fs;
use tracing::info;

use crate::{
    utils::{format_mode, OCI_REPO_SUBDIR},
    MonocoreError, MonocoreResult,
};
use futures::future::join_all;
use std::collections::HashMap;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const WHITEOUT_PREFIX: &str = ".wh.";
const WHITEOUT_OPAQUE: &str = ".wh..wh..opq";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Handles merging of OCI image layers using overlayfs on Linux and copy-merge on other platforms
pub struct OverlayFsMerger {
    /// Path to the actual OCI directory containing repositories and layers.
    oci_dir: PathBuf,

    /// Path where the merged rootfs will be stored
    dest_dir: PathBuf,

    /// Tracks if overlayfs is mounted (Linux only)
    #[cfg(target_os = "linux")]
    is_mounted: bool,
}

/// Tracks original permissions for paths that were temporarily made writable
struct PermissionGuard {
    /// Maps paths to their original permissions
    original_modes: HashMap<PathBuf, u32>,

    /// Paths in order they were modified (for proper restoration)
    modified_paths: Vec<PathBuf>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl OverlayFsMerger {
    /// Creates a new OverlayFsMerger instance
    pub fn new(oci_dir: impl Into<PathBuf>, dest_dir: impl Into<PathBuf>) -> Self {
        Self {
            oci_dir: oci_dir.into(),
            dest_dir: dest_dir.into(),
            #[cfg(target_os = "linux")]
            is_mounted: false,
        }
    }

    /// Merges OCI layers into a single rootfs
    pub async fn merge(&self, repo_tag: &str) -> MonocoreResult<()> {
        let result = self.merge_internal(repo_tag).await;
        if result.is_err() {
            match self.unmount().await {
                Ok(_) => tracing::info!("Cleanup successful after merge error"),
                Err(e) => tracing::error!("Failed to cleanup after merge error: {:?}", e),
            }
        }
        result
    }

    /// Internal implementation of the merge operation
    async fn merge_internal(&self, repo_tag: &str) -> MonocoreResult<()> {
        // Read manifest to get layer order
        let manifest_path = self
            .oci_dir
            .join(OCI_REPO_SUBDIR)
            .join(repo_tag)
            .join("manifest.json");

        let manifest_contents = fs::read_to_string(&manifest_path).await?;
        let manifest: ImageManifest = serde_json::from_str(&manifest_contents)?;

        // Create destination directory if it doesn't exist
        fs::create_dir_all(&self.dest_dir).await?;

        #[cfg(target_os = "linux")]
        {
            self.merge_overlayfs(&manifest).await
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.merge_copy(&manifest).await
        }
    }

    #[cfg(target_os = "linux")]
    async fn merge_overlayfs(&self, manifest: &ImageManifest) -> MonocoreResult<()> {
        todo!()
    }

    #[cfg(not(target_os = "linux"))]
    async fn merge_copy(&self, manifest: &ImageManifest) -> MonocoreResult<()> {
        // Create merged subdirectory
        let merged_dir = self.dest_dir.join("merged");
        fs::create_dir_all(&merged_dir).await?;

        // Create futures with their indices to maintain order
        let extraction_futures: Vec<_> = manifest
            .layers()
            .iter()
            .enumerate() // Add index to each layer
            .map(|(index, layer)| {
                let layer_path = self.oci_dir.join("layer").join(layer.digest().to_string());
                let extracted_path = layer_path.with_extension("extracted");

                async move {
                    if !extracted_path.exists() {
                        info!("Extracting layer {}: {}", index, layer.digest());
                        self.extract_layer(&layer_path, &extracted_path).await?;
                    }
                    Ok::<(usize, PathBuf), MonocoreError>((index, extracted_path))
                }
            })
            .collect();

        // Wait for all extractions to complete and sort by index
        let mut extracted_paths = join_all(extraction_futures)
            .await
            .into_iter()
            .collect::<MonocoreResult<Vec<_>>>()?;

        // Sort by index to maintain layer order
        extracted_paths.sort_by_key(|(idx, _)| *idx);

        // Apply layers in order, discarding the indices
        for (_, path) in extracted_paths {
            self.apply_layer(&path, &merged_dir).await?;
        }

        tracing::debug!("Merged layers into {}", merged_dir.display());

        Ok(())
    }

    /// Extracts a tar.gz layer to the specified path
    async fn extract_layer(&self, layer_path: &Path, extract_path: &Path) -> MonocoreResult<()> {
        fs::create_dir_all(extract_path).await?;

        // Clone paths for the blocking task
        let layer_path = layer_path.to_path_buf();
        let extract_path = extract_path.to_path_buf();

        // Run the blocking tar extraction in a blocking task
        tokio::task::spawn_blocking(move || -> MonocoreResult<()> {
            let tar_gz =
                std::fs::File::open(&layer_path).map_err(|e| MonocoreError::LayerHandling {
                    source: e,
                    layer: layer_path.display().to_string(),
                })?;

            let tar = flate2::read::GzDecoder::new(std::io::BufReader::new(tar_gz));
            let mut archive = tar::Archive::new(tar);

            archive
                .unpack(&extract_path)
                .map_err(|e| MonocoreError::LayerHandling {
                    source: e,
                    layer: layer_path.display().to_string(),
                })?;

            Ok(())
        })
        .await
        .map_err(|e| MonocoreError::LayerExtraction(format!("Join error: {}", e)))??;

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    /// Applies a layer's changes to the destination directory (for non-Linux systems)
    async fn apply_layer(&self, layer_path: &Path, dest_path: &Path) -> MonocoreResult<()> {
        tracing::debug!("Applying layer: {}", layer_path.display());
        let mut stack = vec![layer_path.to_path_buf()];
        let mut perm_guard = PermissionGuard::new();

        while let Some(current_path) = stack.pop() {
            // Make current directory readable to list contents
            perm_guard.make_readable_writable(&current_path).await?;

            let mut entries =
                fs::read_dir(&current_path)
                    .await
                    .map_err(|e| MonocoreError::LayerHandling {
                        source: e,
                        layer: layer_path.display().to_string(),
                    })?;

            let target_dir = dest_path.join(current_path.strip_prefix(layer_path).unwrap());
            fs::create_dir_all(&target_dir)
                .await
                .map_err(MonocoreError::Io)?;

            while let Some(entry) =
                entries
                    .next_entry()
                    .await
                    .map_err(|e| MonocoreError::LayerHandling {
                        source: e,
                        layer: layer_path.display().to_string(),
                    })?
            {
                let path = entry.path();
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Handle whiteouts
                if file_name_str.starts_with(WHITEOUT_PREFIX) {
                    if file_name_str == WHITEOUT_OPAQUE {
                        let target_dir =
                            dest_path.join(current_path.strip_prefix(layer_path).unwrap());

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
                            let relative_path = sibling_path.strip_prefix(layer_path).unwrap();
                            let target_path = dest_path.join(relative_path);

                            Self::handle_fs_entry(&sibling_path, &target_path, &perm_guard).await?;
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
                let relative_path = path.strip_prefix(layer_path).unwrap();
                let target_path = dest_path.join(relative_path);

                // Make source readable and target's parent writable
                perm_guard.make_readable_writable(&path).await?;
                if let Some(parent) = target_path.parent() {
                    perm_guard.make_writable(parent).await?;
                }

                Self::handle_fs_entry(&path, &target_path, &perm_guard).await?;
                if fs::symlink_metadata(&path).await?.file_type().is_dir() {
                    stack.push(path);
                }
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    /// Handles copying, creating directories, or creating symlinks from source to target path
    async fn handle_fs_entry(
        source_path: &Path,
        target_path: &Path,
        perm_guard: &PermissionGuard,
    ) -> MonocoreResult<()> {
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
            .original_modes
            .get(source_path)
            .copied()
            .unwrap_or_else(|| metadata.permissions().mode());

        fs::set_permissions(target_path, std::fs::Permissions::from_mode(original_mode)).await?;
        tracing::debug!(
            "Applied original permissions to {}: {} ({:#o})",
            target_path.display(),
            format_mode(original_mode),
            original_mode
        );

        Ok(())
    }

    /// Unmounts overlayfs on Linux, cleans up files on other platforms
    pub async fn unmount(&self) -> MonocoreResult<()> {
        let merged_dir = self.dest_dir.join("merged");
        if merged_dir.exists() {
            let mut perm_guard = PermissionGuard::new();

            // Recursively check and fix permissions where needed
            let mut stack = vec![merged_dir.clone()];
            while let Some(current_path) = stack.pop() {
                // Check current permissions
                if let Ok(metadata) = fs::metadata(&current_path).await {
                    let mode = metadata.permissions().mode();
                    let is_readable = mode & 0o444 != 0; // Check read permission
                    let is_writable = mode & 0o222 != 0; // Check write permission
                    let is_executable = mode & 0o111 != 0; // Check execute permission for directories

                    // Only modify permissions if necessary
                    if !is_readable || !is_writable || (!is_executable && metadata.is_dir()) {
                        tracing::debug!(
                            "Fixing permissions for {}: {} ({:#o}) - readable: {}, writable: {}, executable: {}",
                            current_path.display(),
                            format_mode(mode),
                            mode,
                            is_readable,
                            is_writable,
                            is_executable
                        );
                        perm_guard.make_readable_writable(&current_path).await?;
                    }
                } else {
                    tracing::warn!("Could not read metadata for {}", current_path.display());
                    // Still try to make it accessible as a fallback
                    perm_guard.make_readable_writable(&current_path).await?;
                }

                // Process directory contents
                if let Ok(mut entries) = fs::read_dir(&current_path).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if path.is_dir() {
                            stack.push(path);
                        }
                    }
                } else {
                    tracing::warn!(
                        "Could not read directory contents of {}",
                        current_path.display()
                    );
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
}

impl PermissionGuard {
    fn new() -> Self {
        Self {
            original_modes: HashMap::new(),
            modified_paths: Vec::new(),
        }
    }

    /// Saves original permissions and makes path temporarily writable
    async fn make_writable(&mut self, path: &Path) -> MonocoreResult<()> {
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
                    format_mode(mode),
                    format_mode(wx_mode),
                    mode,
                    wx_mode
                );
            }
        }
        Ok(())
    }

    /// Makes path readable and writable
    async fn make_readable_writable(&mut self, path: &Path) -> MonocoreResult<()> {
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
                    format_mode(mode),
                    format_mode(rwx_mode),
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
                        format_mode(original_mode),
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_oci_merge_whiteout_handling() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;

        // Create test layers and get repo tag
        let repo_tag =
            helper::create_test_layers_with_whiteouts(&temp_dir.path().to_path_buf()).await?;

        // Setup merger
        let dest_dir = temp_dir.path().join("merged_whiteout_test");
        fs::create_dir_all(&dest_dir).await?;

        let merger = OverlayFsMerger::new(temp_dir.path(), dest_dir.clone());

        // Merge layers using the standard merge function
        merger.merge(&repo_tag).await?;

        // Verify regular whiteout
        let merged_dir = dest_dir.join("merged");
        assert!(
            !merged_dir.join("file1.txt").exists(),
            "file1.txt should be removed by whiteout"
        );
        assert!(
            merged_dir.join("file2.txt").exists(),
            "file2.txt should still exist"
        );
        assert!(
            merged_dir.join("file3.txt").exists(),
            "file3.txt should exist"
        );

        // Verify opaque whiteout
        let dir1 = merged_dir.join("dir1");
        assert!(dir1.exists(), "dir1 should still exist");
        assert!(
            !dir1.join("inside1.txt").exists(),
            "inside1.txt should be hidden by opaque whiteout"
        );
        assert!(
            !dir1.join("inside2.txt").exists(),
            "inside2.txt should be hidden by opaque whiteout"
        );
        assert!(
            dir1.join("new_file.txt").exists(),
            "new_file.txt should exist"
        );

        // Cleanup
        merger.unmount().await?;
        Ok(())
    }

    #[test_log::test(tokio::test)]
    async fn test_oci_merge_permissions_handling() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;

        // Create test layers and get repo tag
        let repo_tag =
            helper::create_test_layers_with_permissions(&temp_dir.path().to_path_buf()).await?;

        // Setup merger
        let dest_dir = temp_dir.path().join("merged_permissions_test");
        fs::create_dir_all(&dest_dir).await?;

        let merger = OverlayFsMerger::new(temp_dir.path(), dest_dir.clone());

        // Merge layers
        merger.merge(&repo_tag).await?;

        // Verify the merged results
        let merged_dir = dest_dir.join("merged");

        // Helper function to verify permissions
        let verify_perms = |path: &Path, expected_mode: u32| -> anyhow::Result<()> {
            let metadata = std::fs::metadata(path)?;
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(
                mode,
                expected_mode,
                "Permission mismatch for {}: expected {} ({:#o}), got {} ({:#o})",
                path.display(),
                format_mode(expected_mode),
                expected_mode,
                format_mode(mode),
                mode
            );
            Ok(())
        };

        // First verify all top-level files and directories
        // These should all be accessible since merged_dir has standard permissions

        // Test write-only file permissions
        let no_read_file = merged_dir.join("no_read_file.txt");
        assert!(no_read_file.exists());
        verify_perms(&no_read_file, 0o200)?; // write-only

        // Test directory with execute-only permission
        let no_read_dir = merged_dir.join("no_read_dir");
        assert!(no_read_dir.exists());
        verify_perms(&no_read_dir, 0o311)?; // --x--x--x

        // Test read-only directory
        let no_write_dir = merged_dir.join("no_write_dir");
        assert!(no_write_dir.exists());
        verify_perms(&no_write_dir, 0o555)?; // r-xr-xr-x

        // Test directory with no permissions
        let no_perm_dir = merged_dir.join("no_perm_dir");
        assert!(no_perm_dir.exists());
        verify_perms(&no_perm_dir, 0o000)?; // ---------

        // Test blocked directory
        let blocked_dir = merged_dir.join("blocked_dir");
        assert!(blocked_dir.exists());
        verify_perms(&blocked_dir, 0o000)?; // ---------

        // Now check contents of directories that we can access

        // Check contents of read-only directory (we have rx permissions)
        let write_protected_file = no_write_dir.join("protected.txt");
        assert!(write_protected_file.exists());
        verify_perms(&write_protected_file, 0o444)?; // read-only
        let content = fs::read_to_string(&write_protected_file).await?;
        assert_eq!(content, "write protected content");

        // Test symlinks (these are in the merged_dir which we can access)
        let symlink = merged_dir.join("link_to_file.txt");
        assert!(symlink.exists());
        assert!(fs::symlink_metadata(&symlink)
            .await?
            .file_type()
            .is_symlink());

        let relative_symlink = merged_dir.join("relative_link.txt");
        assert!(relative_symlink.exists());
        assert!(fs::symlink_metadata(&relative_symlink)
            .await?
            .file_type()
            .is_symlink());

        // Test target files (in merged_dir which we can access)
        let target = merged_dir.join("target.txt");
        if fs::metadata(&target).await?.permissions().mode() & 0o444 != 0 {
            let content = fs::read_to_string(&symlink).await?;
            assert_eq!(content, "target file content");
        }

        // Test FIFO files
        let fifo = merged_dir.join("test.fifo");
        assert!(fifo.exists());
        assert!(fs::symlink_metadata(&fifo).await?.file_type().is_fifo());
        verify_perms(&fifo, 0o644)?; // standard fifo perms

        // Cleanup
        merger.unmount().await?;

        Ok(())
    }

    mod helper {
        use std::str::FromStr;

        use flate2::{write::GzEncoder, Compression};
        use oci_spec::image::{DescriptorBuilder, ImageManifestBuilder, Sha256Digest};
        use tar::Builder;

        use crate::utils::{OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME};

        use super::*;

        /// Creates test layers with whiteout files for testing overlayfs functionality.
        ///
        /// The function creates a three-layer test structure:
        /// ```text
        /// oci/
        /// ├── layer/
        /// │   ├── sha256:1111... (Layer 1 - Base)
        /// │   │   ├── file1.txt         ("original content")
        /// │   │   ├── file2.txt         ("keep this file")
        /// │   │   └── dir1/
        /// │   │       ├── inside1.txt   ("inside1")
        /// │   │       └── inside2.txt   ("inside2")
        /// │   │
        /// │   ├── sha256:2222... (Layer 2 - Regular Whiteout)
        /// │   │   ├── .wh.file1.txt    (removes file1.txt)
        /// │   │   └── file3.txt        ("new file")
        /// │   │
        /// │   └── sha256:3333... (Layer 3 - Opaque Whiteout)
        /// │       └── dir1/
        /// │           ├── .wh..wh..opq  (hides all contents of dir1)
        /// │           └── new_file.txt  ("new content")
        /// │
        /// └── repo/
        ///     └── test_layers/
        ///         └── manifest.json
        /// ```
        ///
        /// After merging these layers:
        /// - file1.txt will be removed (due to whiteout in Layer 2)
        /// - file2.txt will remain with original content
        /// - file3.txt will be added from Layer 2
        /// - dir1's original contents (inside1.txt, inside2.txt) will be hidden
        /// - dir1 will only contain new_file.txt from Layer 3
        pub(super) async fn create_test_layers_with_whiteouts(
            base_dir: &PathBuf,
        ) -> anyhow::Result<String> {
            // use monocore::utils::{OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME, OCI_REPO_SUBDIR};
            use serde_json::to_string_pretty;

            // Create OCI directory structure
            let oci_dir = base_dir;
            let layers_dir = oci_dir.join(OCI_LAYER_SUBDIR);
            let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join("test_layers");

            for dir in [&layers_dir, &repo_dir] {
                fs::create_dir_all(dir).await?;
            }

            // Create layer directories and their content
            let layer_digests = vec![
                "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
                "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
                "sha256:3333333333333333333333333333333333333333333333333333333333333333"
                    .to_string(),
            ];

            // Create temporary directory for layer contents
            let temp_dir = tempdir()?;

            // Layer 1: Base files
            {
                let layer_contents = temp_dir.path().join("layer1");
                fs::create_dir_all(&layer_contents).await?;
                fs::write(layer_contents.join("file1.txt"), "original content").await?;
                fs::write(layer_contents.join("file2.txt"), "keep this file").await?;
                fs::create_dir(layer_contents.join("dir1")).await?;
                fs::write(layer_contents.join("dir1/inside1.txt"), "inside1").await?;
                fs::write(layer_contents.join("dir1/inside2.txt"), "inside2").await?;

                // Create tar.gz for layer 1
                let layer_file = std::fs::File::create(layers_dir.join(&layer_digests[0]))?;
                let encoder = GzEncoder::new(layer_file, Compression::default());
                let mut tar = Builder::new(encoder);
                tar.append_dir_all(".", layer_contents)?;
                tar.finish()?;
            }

            // Layer 2: Regular whiteout
            {
                let layer_contents = temp_dir.path().join("layer2");
                fs::create_dir_all(&layer_contents).await?;
                fs::write(layer_contents.join(".wh.file1.txt"), "").await?;
                fs::write(layer_contents.join("file3.txt"), "new file").await?;

                // Create tar.gz for layer 2
                let layer_file = std::fs::File::create(layers_dir.join(&layer_digests[1]))?;
                let encoder = GzEncoder::new(layer_file, Compression::default());
                let mut tar = Builder::new(encoder);
                tar.append_dir_all(".", layer_contents)?;
                tar.finish()?;
            }

            // Layer 3: Opaque whiteout
            {
                let layer_contents = temp_dir.path().join("layer3");
                fs::create_dir_all(&layer_contents).await?;
                fs::create_dir(layer_contents.join("dir1")).await?;
                fs::write(layer_contents.join("dir1/.wh..wh..opq"), "").await?;
                fs::write(layer_contents.join("dir1/new_file.txt"), "new content").await?;

                // Create tar.gz for layer 3
                let layer_file = std::fs::File::create(layers_dir.join(&layer_digests[2]))?;
                let encoder = GzEncoder::new(layer_file, Compression::default());
                let mut tar = Builder::new(encoder);
                tar.append_dir_all(".", layer_contents)?;
                tar.finish()?;
            }

            // Create manifest
            let manifest = ImageManifestBuilder::default()
                .schema_version(2_u32)
                .config(
                    DescriptorBuilder::default()
                        .media_type("application/vnd.oci.image.config.v1+json")
                        .digest(
                            Sha256Digest::from_str(
                                "1111111111111111111111111111111111111111111111111111111111111111",
                            )
                            .expect("Invalid config digest"),
                        )
                        .size(0_u64)
                        .build()
                        .unwrap(),
                )
                .layers(
                    layer_digests
                        .iter()
                        .map(|digest_str| {
                            let digest =
                                Sha256Digest::from_str(digest_str.trim_start_matches("sha256:"))
                                    .expect("Invalid digest");

                            DescriptorBuilder::default()
                                .media_type("application/vnd.oci.image.layer.v1.tar+gzip")
                                .digest(digest)
                                .size(0_u64)
                                .build()
                                .unwrap()
                        })
                        .collect::<Vec<_>>(),
                )
                .build()?;

            // Write manifest
            let manifest_path = repo_dir.join(OCI_MANIFEST_FILENAME);
            fs::write(&manifest_path, to_string_pretty(&manifest)?).await?;

            Ok("test_layers".to_string())
        }

        /// Creates test layers with various permission scenarios for testing overlayfs functionality.
        ///
        /// This function creates a two-layer test structure to verify handling of:
        /// - Files and directories with restricted permissions
        /// - Special files (FIFOs)
        /// - Symlinks (both absolute and relative)
        /// - Nested directories with mixed permissions
        ///
        /// Layer Structure:
        /// ```text
        /// Layer 1 (Base Layer - sha256:4444...)
        /// ├── no_read_file.txt     (0o200, w-------)  "no read permission content"
        /// ├── target.txt           (0o644, rw-r--r--) "target file content"
        /// ├── test.fifo            (0o644, rw-r--r--) [named pipe]
        /// ├── no_read_dir/         (0o311, --x--x--x)
        /// │   └── inside.txt       (0o644, rw-r--r--) "inside no-read directory"
        /// ├── no_write_dir/        (0o555, r-xr-xr-x)
        /// │   └── protected.txt    (0o444, r--r--r--) "write protected content"
        /// ├── no_perm_dir/         (0o000, ---------)
        /// │   └── hidden.txt       (0o644, rw-r--r--) "hidden content"
        /// ├── blocked_dir/         (0o000, ---------)
        /// │   └── inner_dir/       (0o777, rwxrwxrwx)
        /// │       └── nested.txt   (0o666, rw-rw-rw-) "nested file content"
        /// ├── subdir/              (0o755, rwxr-xr-x)
        /// │   └── target.txt       (0o644, rw-r--r--) "target in subdirectory"
        ///
        /// Layer 2 (Symlinks Layer - sha256:5555...)
        /// ├── link_to_file.txt     -> target.txt
        /// └── relative_link.txt    -> subdir/target.txt
        /// ```
        ///
        /// Test Coverage:
        /// 1. Permission Combinations:
        ///    - Write-only file (no read access)
        ///    - Directory with execute-only (no read/write)
        ///    - Directory with read+execute (no write)
        ///    - Directory with no permissions
        ///    - Read-only file in read-only directory
        ///    - Nested directory structure with:
        ///      * Outer directory having no permissions (0o000)
        ///      * Inner directory with full permissions (0o777)
        ///      * Regular file with read-write permissions (0o666)
        ///
        /// 2. Special Files:
        ///    - Named pipe (FIFO)
        ///    - Absolute symlink
        ///    - Relative symlink across directories
        ///
        /// 3. Access Patterns:
        ///    - Reading through write-only files
        ///    - Traversing no-read directories
        ///    - Accessing files in no-permission directories
        ///    - Following symlinks through restricted directories
        ///    - Accessing nested files through blocked parent directories
        pub(super) async fn create_test_layers_with_permissions(
            base_dir: &PathBuf,
        ) -> anyhow::Result<String> {
            use std::os::unix::fs::symlink;

            // Create OCI directory structure
            let oci_dir = base_dir;
            let layers_dir = oci_dir.join(OCI_LAYER_SUBDIR);
            let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join("test_permissions");

            for dir in [&layers_dir, &repo_dir] {
                fs::create_dir_all(dir).await?;
            }

            let layer_digests = vec![
                "sha256:4444444444444444444444444444444444444444444444444444444444444444"
                    .to_string(),
                "sha256:5555555555555555555555555555555555555555555555555555555555555555"
                    .to_string(),
            ];

            // Layer 1: Base layer with various permission scenarios
            {
                // Create the extracted layer directory directly
                let layer_path = layers_dir
                    .join(&layer_digests[0])
                    .with_extension("extracted");
                fs::create_dir_all(&layer_path).await?;

                // Create a write-only file (no read permission)
                let no_read_file = layer_path.join("no_read_file.txt");
                fs::write(&no_read_file, "no read permission content").await?;
                fs::set_permissions(&no_read_file, std::fs::Permissions::from_mode(0o200)).await?;

                // Create a directory with no read permission (execute only)
                let no_read_dir = layer_path.join("no_read_dir");
                fs::create_dir(&no_read_dir).await?;
                fs::write(no_read_dir.join("inside.txt"), "inside no-read directory").await?;
                fs::set_permissions(&no_read_dir, std::fs::Permissions::from_mode(0o311)).await?;

                // Create a read-only directory (no write permission)
                let no_write_dir = layer_path.join("no_write_dir");
                fs::create_dir(&no_write_dir).await?;
                fs::write(
                    no_write_dir.join("protected.txt"),
                    "write protected content",
                )
                .await?;
                fs::set_permissions(&no_write_dir, std::fs::Permissions::from_mode(0o555)).await?;
                fs::set_permissions(
                    &no_write_dir.join("protected.txt"),
                    std::fs::Permissions::from_mode(0o444),
                )
                .await?;

                // Create a directory with no permissions
                let no_perm_dir = layer_path.join("no_perm_dir");
                fs::create_dir(&no_perm_dir).await?;
                fs::write(no_perm_dir.join("hidden.txt"), "hidden content").await?;
                fs::set_permissions(&no_perm_dir, std::fs::Permissions::from_mode(0o000)).await?;

                // Create target files for symlinks
                fs::write(layer_path.join("target.txt"), "target file content").await?;
                fs::create_dir_all(layer_path.join("subdir")).await?;
                fs::write(
                    layer_path.join("subdir").join("target.txt"),
                    "target in subdirectory",
                )
                .await?;

                // Create FIFO file
                let fifo_path = layer_path.join("test.fifo");
                unistd::mkfifo(&fifo_path, Mode::from_bits_truncate(0o644))?;

                // Create nested directory structure with mixed permissions
                let blocked_dir = layer_path.join("blocked_dir");
                let inner_dir = blocked_dir.join("inner_dir");
                fs::create_dir_all(&inner_dir).await?;
                fs::write(inner_dir.join("nested.txt"), "nested file content").await?;
                fs::set_permissions(
                    inner_dir.join("nested.txt"),
                    std::fs::Permissions::from_mode(0o666),
                )
                .await?;
                fs::set_permissions(&inner_dir, std::fs::Permissions::from_mode(0o777)).await?;
                fs::set_permissions(&blocked_dir, std::fs::Permissions::from_mode(0o000)).await?;

                // Create empty tar.gz file just to satisfy the manifest
                let layer_file = std::fs::File::create(layers_dir.join(&layer_digests[0]))?;
                let encoder = GzEncoder::new(layer_file, Compression::default());
                let mut tar = Builder::new(encoder);
                tar.finish()?;
            }

            // Layer 2: Add symlinks
            {
                // Create the extracted layer directory directly
                let layer_path = layers_dir
                    .join(&layer_digests[1])
                    .with_extension("extracted");
                fs::create_dir_all(&layer_path).await?;

                // Create absolute symlink to target.txt
                symlink("target.txt", layer_path.join("link_to_file.txt"))?;

                // Create relative symlink to file in subdirectory
                symlink("subdir/target.txt", layer_path.join("relative_link.txt"))?;

                // Create empty tar.gz file just to satisfy the manifest
                let layer_file = std::fs::File::create(layers_dir.join(&layer_digests[1]))?;
                let encoder = GzEncoder::new(layer_file, Compression::default());
                let mut tar = Builder::new(encoder);
                tar.finish()?;
            }

            // Create manifest
            let manifest = ImageManifestBuilder::default()
                .schema_version(2_u32)
                .config(
                    DescriptorBuilder::default()
                        .media_type("application/vnd.oci.image.config.v1+json")
                        .digest(
                            Sha256Digest::from_str(
                                "4444444444444444444444444444444444444444444444444444444444444444",
                            )
                            .expect("Invalid config digest"),
                        )
                        .size(0_u64)
                        .build()
                        .unwrap(),
                )
                .layers(
                    layer_digests
                        .iter()
                        .map(|digest_str| {
                            let digest =
                                Sha256Digest::from_str(digest_str.trim_start_matches("sha256:"))
                                    .expect("Invalid digest");

                            DescriptorBuilder::default()
                                .media_type("application/vnd.oci.image.layer.v1.tar+gzip")
                                .digest(digest)
                                .size(0_u64)
                                .build()
                                .unwrap()
                        })
                        .collect::<Vec<_>>(),
                )
                .build()?;

            // Write manifest
            let manifest_path = repo_dir.join(OCI_MANIFEST_FILENAME);
            fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?).await?;

            Ok("test_permissions".to_string())
        }
    }
}

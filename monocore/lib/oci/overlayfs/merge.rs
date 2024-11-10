use std::{
    io,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

#[cfg(target_os = "linux")]
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::{sys::stat::Mode, unistd};
use oci_spec::image::ImageManifest;
use tokio::fs;
use tracing::info;

use crate::{utils::OCI_REPO_SUBDIR, MonocoreError, MonocoreResult};
use futures::future::join_all;

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

//--------------------------------------------------------------------------------------------------
// Implementation
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

    /// Handles copying, creating directories, or creating symlinks from source to target path
    async fn handle_fs_entry(source_path: &Path, target_path: &Path) -> io::Result<()> {
        // Create parent directory if needed
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }

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
        // Block/character devices are intentionally not supported for security reasons
        // They would require elevated privileges to create

        // Copy permissions
        let permissions = metadata.permissions();
        fs::set_permissions(target_path, permissions).await?;

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    /// Applies a layer's changes to the destination directory (for non-Linux systems)
    async fn apply_layer(&self, layer_path: &Path, dest_path: &Path) -> io::Result<()> {
        tracing::debug!("Applying layer: {}", layer_path.display());
        let mut stack = vec![layer_path.to_path_buf()];

        while let Some(current_path) = stack.pop() {
            let mut entries = fs::read_dir(&current_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Handle whiteouts
                if file_name_str.starts_with(WHITEOUT_PREFIX) {
                    if file_name_str == WHITEOUT_OPAQUE {
                        // Get the directory containing the opaque whiteout
                        let target_dir =
                            dest_path.join(current_path.strip_prefix(layer_path).unwrap());
                        if target_dir.exists() {
                            fs::remove_dir_all(&target_dir).await?;
                        }
                        fs::create_dir_all(&target_dir).await?;

                        // Process the remaining files in this directory
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

                            Self::handle_fs_entry(&sibling_path, &target_path).await?;
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
                        let target_path = dest_path
                            .join(current_path.strip_prefix(layer_path).unwrap())
                            .join(original_name);

                        if target_path.exists() {
                            if target_path.is_dir() {
                                fs::remove_dir_all(&target_path).await?;
                            } else {
                                fs::remove_file(&target_path).await?;
                            }
                        }
                    }
                    continue;
                }

                // Copy regular files and directories
                let relative_path = path.strip_prefix(layer_path).unwrap();
                let target_path = dest_path.join(relative_path);

                Self::handle_fs_entry(&path, &target_path).await?;
                if fs::symlink_metadata(&path).await?.file_type().is_dir() {
                    stack.push(path);
                }
            }
        }

        Ok(())
    }

    /// Unmounts overlayfs on Linux, cleans up files on other platforms
    pub async fn unmount(&self) -> MonocoreResult<()> {
        let merged_dir = self.dest_dir.join("merged");
        if merged_dir.exists() {
            fs::remove_dir_all(&merged_dir).await?;
        }
        Ok(())
    }
}

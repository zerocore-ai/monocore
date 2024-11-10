use std::{
    fs::File,
    io::ErrorKind,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use futures::future;
#[cfg(target_os = "linux")]
use nix::mount::{mount, umount2, MntFlags};
#[cfg(not(target_os = "linux"))]
use nix::sys::stat::{mknod, Mode, SFlag};
use oci_spec::image::ImageManifest;
use serde_json::from_str;
use tar::Archive;
use tokio::fs;
use tracing;
use walkdir::WalkDir;

use crate::{
    utils::{OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME, OCI_REPO_SUBDIR},
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Handles merging of OCI image layers using overlayfs
pub struct OverlayFsMerger {
    /// Path to the OCI directory containing layers and metadata
    oci_dir: PathBuf,

    /// Path where the merged rootfs will be stored
    dest_dir: PathBuf,
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
        }
    }

    /// Ensures layer is extracted and returns the extracted path
    async fn ensure_layer_extracted(&self, layer_path: &Path) -> MonocoreResult<PathBuf> {
        let extracted_path = layer_path.with_extension("extracted");

        if !extracted_path.exists() {
            tracing::debug!(?layer_path, "Extracting layer");

            match fs::create_dir_all(&extracted_path).await {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    return Ok(extracted_path);
                }
                Err(e) => return Err(e.into()),
            }

            let layer_path_for_task = layer_path.to_path_buf();
            let extracted_path_clone = extracted_path.clone();

            tokio::task::spawn_blocking(move || -> MonocoreResult<()> {
                let tar_gz = File::open(&layer_path_for_task)?;
                let tar = GzDecoder::new(tar_gz);
                let mut archive = Archive::new(tar);

                archive.set_preserve_permissions(true);
                archive.set_preserve_mtime(true);
                archive.set_unpack_xattrs(true);

                // First pass: Extract all regular files
                let entries = archive.entries()?;
                for entry in entries {
                    let mut entry = entry?;
                    let path = entry.path()?.into_owned();
                    let target = extracted_path_clone.join(&path);

                    // Skip hard links in first pass
                    if entry.header().entry_type().is_hard_link() {
                        continue;
                    }

                    // Create parent directories if needed
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    // Extract the file with preserved attributes
                    entry.unpack(&target)?;

                    // Handle whiteout files
                    if let Some(file_name) = path.file_name() {
                        if let Some(name_str) = file_name.to_str() {
                            if name_str.starts_with(".wh.") {
                                use std::os::unix::fs::PermissionsExt;
                                let mut perms = std::fs::metadata(&target)?.permissions();
                                perms.set_mode(0o000);
                                std::fs::set_permissions(&target, perms)?;
                            }
                        }
                    }
                }

                // Second pass: Handle hard links
                let entries = archive.entries()?;
                for entry in entries {
                    let entry = entry?;
                    if entry.header().entry_type().is_hard_link() {
                        let path = entry.path()?.into_owned();
                        let target = extracted_path_clone.join(&path);

                        // Get the link name (original file)
                        if let Some(link_name) = entry.link_name()? {
                            let link_target = extracted_path_clone.join(link_name);

                            // Create parent directories if needed
                            if let Some(parent) = target.parent() {
                                std::fs::create_dir_all(parent)?;
                            }

                            // Create the hard link
                            if let Err(e) = std::fs::hard_link(&link_target, &target) {
                                tracing::warn!(
                                    ?target,
                                    ?link_target,
                                    error = ?e,
                                    "Failed to create hard link - copying file instead"
                                );
                                // Fall back to copying the file if hard linking fails
                                std::fs::copy(&link_target, &target)?;
                            }
                        }
                    }
                }

                Ok(())
            })
            .await??;
        }

        Ok(extracted_path)
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

    async fn merge_internal(&self, repo_tag: &str) -> MonocoreResult<()> {
        tracing::info!(?repo_tag, "Starting layer merge");

        // Read manifest to get layer order
        let manifest_path = self
            .oci_dir
            .join(OCI_REPO_SUBDIR)
            .join(repo_tag)
            .join(OCI_MANIFEST_FILENAME);

        tracing::debug!(?manifest_path, "Reading manifest");
        let manifest_content = fs::read_to_string(&manifest_path).await?;
        let manifest: ImageManifest = from_str(&manifest_content)?;

        // Create required directories
        let work_dir = self.dest_dir.join("work");
        let upper_dir = self.dest_dir.join("upper");
        let merged_dir = self.dest_dir.join("merged");

        tracing::debug!("Creating overlay directories");
        for dir in [&work_dir, &upper_dir, &merged_dir] {
            fs::create_dir_all(dir).await?;
        }

        // Get layer paths and extract in parallel
        let layer_dir = self.oci_dir.join(OCI_LAYER_SUBDIR);
        let layer_paths: Vec<_> = manifest
            .layers()
            .iter()
            .map(|layer| layer_dir.join(layer.digest().to_string()))
            .collect();

        tracing::debug!("Extracting layers in parallel");
        let lower_dirs = self.extract_layers(&layer_paths).await?;

        // Ensure all extracted directories exist before proceeding
        for dir in &lower_dirs {
            if !dir.exists() {
                return Err(MonocoreError::from(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Extracted layer directory not found: {}", dir.display()),
                )));
            }
        }

        #[cfg(target_os = "linux")]
        {
            tracing::debug!("Mounting overlayfs");
            let lower = lower_dirs
                .iter()
                .rev() // Reverse order for proper layer stacking
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join(":");
            self.mount_overlayfs(&lower, &upper_dir, &work_dir, &merged_dir)?;
            tracing::info!("Overlayfs mounted successfully");
        }

        #[cfg(not(target_os = "linux"))]
        {
            tracing::debug!("Performing copy-merge for non-Linux system");
            self.copy_merge(&lower_dirs, &merged_dir).await?;
            tracing::info!("Copy-merge completed successfully");
        }

        Ok(())
    }

    /// Mounts the overlayfs with the specified directories
    #[cfg(target_os = "linux")]
    fn mount_overlayfs(
        &self,
        lower: &str,
        upper: &Path,
        work: &Path,
        merged: &Path,
    ) -> MonocoreResult<()> {
        let options = format!(
            "lowerdir={},upperdir={},workdir={},index=off,userxattr",
            lower,
            upper.display(),
            work.display()
        );

        // Use MsFlags instead of MntFlags
        mount(
            Some("overlay"),
            merged,
            Some("overlay"),
            MsFlags::MS_NODEV | MsFlags::MS_NOSUID,
            Some(options.as_str()),
        )?;

        Ok(())
    }

    /// Unmounts the overlayfs and cleans up
    pub async fn unmount(&self) -> MonocoreResult<()> {
        tracing::debug!("Starting unmount process");

        #[cfg(target_os = "linux")]
        {
            use std::io::Error as IoError;

            let merged_dir = self.dest_dir.join("merged");
            tracing::debug!(?merged_dir, "Unmounting overlayfs");
            // Use MsFlags::MS_FORCE for force unmount
            if let Err(e) = nix::sys::mount::umount2(&merged_dir, MsFlags::MS_FORCE) {
                tracing::error!(?merged_dir, error = ?e, "Failed to unmount overlayfs");
                return Err(IoError::new(ErrorKind::Other, e).into());
            }
        }

        // Clean up directories (this works for both Linux and non-Linux)
        tracing::debug!("Cleaning up overlay directories");
        for dir in ["work", "upper", "merged"] {
            let path = self.dest_dir.join(dir);
            if path.exists() {
                if let Err(e) = fs::remove_dir_all(&path).await {
                    tracing::warn!(?path, error = ?e, "Failed to remove directory");
                }
            }
        }

        tracing::info!("Unmount and cleanup completed");
        Ok(())
    }

    /// Copies and merges layers for non-Linux systems
    #[cfg(not(target_os = "linux"))]
    async fn copy_merge(&self, lower_dirs: &[PathBuf], merged_dir: &Path) -> MonocoreResult<()> {
        // Create merged directory if it doesn't exist
        fs::create_dir_all(merged_dir).await?;

        // Copy layers in order (from bottom to top)
        for layer_dir in lower_dirs.iter().rev() {
            self.copy_layer(layer_dir, merged_dir).await?;
        }

        Ok(())
    }

    /// Common file handling logic for both Linux and non-Linux platforms
    #[cfg(not(target_os = "linux"))]
    async fn handle_file(
        &self,
        path: &Path,
        target: &Path,
        metadata: &std::fs::Metadata,
        file_type: std::fs::FileType,
    ) -> MonocoreResult<()> {
        if file_type.is_dir() {
            fs::create_dir_all(target).await?;
            self.copy_metadata(path, target).await?;
        } else if file_type.is_file() {
            fs::copy(path, target).await?;
            self.copy_metadata(path, target).await?;
        } else if file_type.is_symlink() {
            let link_target = tokio::fs::read_link(path).await?;
            let target = target.to_path_buf();
            tokio::task::spawn_blocking(move || std::os::unix::fs::symlink(&link_target, &target))
                .await??;
        } else {
            self.handle_special_file(path, target, metadata).await?;
        }
        Ok(())
    }

    /// Handle special files (devices, sockets, FIFOs)
    #[cfg(not(target_os = "linux"))]
    async fn handle_special_file(
        &self,
        path: &Path,
        target: &Path,
        metadata: &std::fs::Metadata,
    ) -> MonocoreResult<()> {
        let mode = metadata.mode() as u16;
        let file_type = mode & libc::S_IFMT;

        match file_type {
            libc::S_IFCHR | libc::S_IFBLK => {
                tracing::debug!(?path, "Skipping device node - will be handled by microVM");
            }
            libc::S_IFIFO => {
                let target = target.to_path_buf();
                tokio::task::spawn_blocking(move || -> Result<(), nix::Error> {
                    let mode = Mode::from_bits_truncate(mode);
                    match mknod(&target, SFlag::S_IFIFO, mode, 0) {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            tracing::warn!(?target, error = ?e, "Failed to create FIFO - continuing");
                            Ok(())
                        }
                    }
                })
                .await??;
            }
            libc::S_IFSOCK => {
                tracing::debug!(
                    ?path,
                    "Skipping Unix domain socket - will be recreated by service"
                );
            }
            _ => {
                tracing::warn!(
                    ?path,
                    file_type = ?file_type,
                    "Unknown special file type - skipping"
                );
            }
        }
        Ok(())
    }

    /// Copies file metadata from source to destination
    #[cfg(not(target_os = "linux"))]
    async fn copy_metadata(&self, src: &Path, dst: &Path) -> MonocoreResult<()> {
        let src = src.to_path_buf();
        let dst = dst.to_path_buf();

        tokio::task::spawn_blocking(move || -> MonocoreResult<()> {
            let metadata = std::fs::metadata(&src)?;

            // Basic permissions
            std::fs::set_permissions(&dst, metadata.permissions())?;

            #[cfg(unix)]
            Self::set_unix_metadata(&dst, &metadata)?;

            Ok(())
        })
        .await??;

        Ok(())
    }

    #[cfg(unix)]
    fn set_unix_metadata(dst: &Path, metadata: &std::fs::Metadata) -> MonocoreResult<()> {
        use nix::sys::stat::Mode;

        // Set mode (including special bits)
        let mode = Mode::from_bits_truncate(metadata.mode() as u16);
        if let Err(e) = nix::sys::stat::fchmodat(
            None,
            dst,
            mode,
            nix::sys::stat::FchmodatFlags::FollowSymlink,
        ) {
            tracing::warn!(?dst, error = ?e, "Failed to set mode bits - continuing");
        }

        // Set timestamps
        if let Ok(mtime) = metadata.modified() {
            if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                let times = nix::sys::time::TimeSpec::from(duration);
                if let Err(e) = nix::sys::stat::utimensat(
                    None,
                    dst,
                    &times,
                    &times,
                    nix::sys::stat::UtimensatFlags::FollowSymlink,
                ) {
                    tracing::warn!(?dst, error = ?e, "Failed to set timestamps - continuing");
                }
            }
        }

        Ok(())
    }

    /// Copies a single layer, handling file overwrites and whiteouts
    #[cfg(not(target_os = "linux"))]
    async fn copy_layer(&self, src: &Path, dst: &Path) -> MonocoreResult<()> {
        tracing::debug!(?src, ?dst, "Copying layer");

        // First pass: Handle whiteouts
        self.handle_whiteouts(src, dst).await?;

        // Second pass: Copy regular files
        for entry in WalkDir::new(src).min_depth(1) {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(src)?;
            let target = dst.join(relative);

            // Skip whiteout files in second pass
            if let Some(file_name) = path.file_name() {
                let file_name = file_name.to_string_lossy();
                if file_name == ".wh..wh..opq" || file_name.starts_with(".wh.") {
                    continue;
                }
            }

            // Ensure parent directory exists
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).await?;
            }

            let metadata = entry.metadata()?;
            self.handle_file(path, &target, &metadata, metadata.file_type())
                .await?;
        }

        tracing::info!(?src, ?dst, "Layer copy completed");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    async fn handle_whiteouts(&self, src: &Path, dst: &Path) -> MonocoreResult<()> {
        for entry in WalkDir::new(src).min_depth(1) {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(src)?;
            let target = dst.join(relative);

            if let Some(file_name) = path.file_name() {
                let file_name = file_name.to_string_lossy();

                if file_name == ".wh..wh..opq" {
                    if let Some(parent) = target.parent() {
                        tracing::debug!(?parent, "Found opaque whiteout, clearing directory");
                        if parent.exists() {
                            fs::remove_dir_all(&parent).await?;
                            fs::create_dir(&parent).await?;
                        }
                    }
                    continue;
                }

                if file_name.starts_with(".wh.") {
                    let original_name = file_name.trim_start_matches(".wh.");
                    let remove_path = target.parent().unwrap().join(original_name);
                    tracing::debug!(?remove_path, "Processing whiteout file");

                    if remove_path.exists() {
                        if remove_path.is_dir() {
                            fs::remove_dir_all(&remove_path).await?;
                        } else {
                            fs::remove_file(&remove_path).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Extracts multiple layers in parallel
    async fn extract_layers(&self, layer_paths: &[PathBuf]) -> MonocoreResult<Vec<PathBuf>> {
        let futures: Vec<_> = layer_paths
            .iter()
            .map(|path| self.ensure_layer_extracted(path))
            .collect();

        future::try_join_all(futures).await
    }
}

use std::path::{Path, PathBuf};

use futures::future::join_all;
use oci_spec::image::ImageManifest;
use tokio::fs;

use crate::{
    utils::{self, MERGED_SUBDIR, OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME, OCI_REPO_SUBDIR},
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const EXTRACTED_LAYER_EXTENSION: &str = "extracted";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Merges OCI image layers into a single rootfs directory.
///
/// Uses copy-based merging by default on all platforms. On Linux with the overlayfs feature
/// enabled, can optionally use overlayfs for more efficient merging.
///
/// # Arguments
/// * `oci_dir` - Base directory containing OCI image data
/// * `dest_dir` - Destination directory for the merged layers
/// * `repo_tag` - Repository tag identifying the image to merge
///
/// # Directory structure
/// Creates the following structure under dest_dir:
/// ```text
/// dest_dir/
/// ├── merged/  - Final merged filesystem
/// ├── upper/   - Overlayfs upper dir (Linux with overlayfs only)
/// └── work/    - Overlayfs work dir (Linux with overlayfs only)
/// ```
pub async fn merge(
    oci_dir: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
    repo_tag: &str,
) -> MonocoreResult<()> {
    let oci_dir = oci_dir.as_ref();
    let dest_dir = dest_dir.as_ref();

    let result = merge_internal(oci_dir, dest_dir, repo_tag).await;
    if result.is_err() {
        match super::unmount(dest_dir).await {
            Ok(_) => tracing::info!("Cleanup successful after merge error"),
            Err(e) => tracing::error!("Failed to cleanup after merge error: {:?}", e),
        }
    }
    result
}

// Linux-specific merge implementation
#[cfg(target_os = "linux")]
async fn merge_internal(oci_dir: &Path, dest_dir: &Path, repo_tag: &str) -> MonocoreResult<()> {
    // Read manifest to get layer order
    let manifest_path = oci_dir
        .join(OCI_REPO_SUBDIR)
        .join(repo_tag)
        .join(OCI_MANIFEST_FILENAME);

    let manifest_contents = fs::read_to_string(&manifest_path).await?;
    let manifest: ImageManifest = serde_json::from_str(&manifest_contents)?;

    // Try overlayfs first if feature is enabled
    #[cfg(feature = "overlayfs")]
    {
        match merge_overlayfs(&manifest, oci_dir, dest_dir).await {
            Ok(()) => {
                tracing::info!("Overlayfs merge successful");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Overlayfs merge failed, falling back to copy merge: {}", e);
            }
        }
    }

    // Fall back to copy merge
    merge_copy(&manifest, oci_dir, dest_dir).await
}

// Non-Linux merge implementation (copy-only)
#[cfg(not(target_os = "linux"))]
async fn merge_internal(oci_dir: &Path, dest_dir: &Path, repo_tag: &str) -> MonocoreResult<()> {
    let manifest_path = oci_dir
        .join(OCI_REPO_SUBDIR)
        .join(repo_tag)
        .join(OCI_MANIFEST_FILENAME);

    let manifest_contents = fs::read_to_string(&manifest_path).await?;
    let manifest: ImageManifest = serde_json::from_str(&manifest_contents)?;

    merge_copy(&manifest, oci_dir, dest_dir).await
}

// Linux-specific overlayfs implementation
#[cfg(all(target_os = "linux", feature = "overlayfs"))]
async fn merge_overlayfs(
    manifest: &ImageManifest,
    oci_dir: &Path,
    dest_dir: &Path,
) -> MonocoreResult<()> {
    use crate::utils::OCI_LAYER_SUBDIR;

    let merged_dir = dest_dir.join(MERGED_SUBDIR);
    fs::create_dir_all(&merged_dir).await?;

    // Create unique work and upper directories using a timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let work_dir = dest_dir.join(format!("work_{}", timestamp));
    let upper_dir = dest_dir.join(format!("upper_{}", timestamp));
    fs::create_dir_all(&work_dir).await?;
    fs::create_dir_all(&upper_dir).await?;

    // Create futures for layer extraction
    let extraction_futures: Vec<_> = manifest
        .layers()
        .iter()
        .rev() // Reverse order for overlayfs
        .map(|layer| {
            let tar_path = oci_dir
                .join(OCI_LAYER_SUBDIR)
                .join(utils::sanitize_name_for_path(&layer.digest().to_string()));

            let layer_path = tar_path.with_extension(EXTRACTED_LAYER_EXTENSION);

            async move {
                if !layer_path.exists() {
                    extract_layer(&tar_path, &layer_path).await?;
                }
                Ok::<String, MonocoreError>(layer_path.to_string_lossy().into_owned())
            }
        })
        .collect();

    // Wait for all extractions to complete
    let lower_dirs = join_all(extraction_futures)
        .await
        .into_iter()
        .collect::<MonocoreResult<Vec<_>>>()?;

    // Build mount options with index=off
    let mount_opts = format!(
        "index=off,lowerdir={},upperdir={},workdir={}",
        lower_dirs.join(":"),
        upper_dir.display(),
        work_dir.display()
    );

    // Try to mount overlayfs
    let status = Command::new("mount")
        .arg("-t")
        .arg("overlay")
        .arg("overlay")
        .arg("-o")
        .arg(&mount_opts)
        .arg(&merged_dir)
        .status()?;

    if !status.success() {
        // Check if overlayfs module is available
        let modules = fs::read_to_string("/proc/filesystems")
            .await
            .unwrap_or_default();
        if !modules.contains("overlay") {
            tracing::error!("Overlayfs not available in kernel");
            return Err(MonocoreError::LayerHandling {
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "overlayfs not supported by kernel",
                ),
                layer: "overlayfs".to_string(),
            });
        }

        tracing::error!("Failed to mount overlayfs with options: {}", mount_opts);
        return Err(MonocoreError::LayerHandling {
            source: std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to mount overlayfs: {}", status),
            ),
            layer: "overlayfs".to_string(),
        });
    }

    Ok(())
}

// Copy-based merge implementation (available on all platforms)
async fn merge_copy(
    manifest: &ImageManifest,
    oci_dir: &Path,
    dest_dir: &Path,
) -> MonocoreResult<()> {
    let merged_dir = dest_dir.join(MERGED_SUBDIR);
    fs::create_dir_all(&merged_dir).await?;

    // Create futures with their indices to maintain order
    let extraction_futures: Vec<_> = manifest
        .layers()
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let layer_path = oci_dir
                .join(OCI_LAYER_SUBDIR)
                .join(utils::sanitize_name_for_path(layer.digest().as_ref()));

            let extracted_path = layer_path.with_extension(EXTRACTED_LAYER_EXTENSION);

            async move {
                if !extracted_path.exists() {
                    tracing::info!("Extracting layer {}: {}", index, layer.digest());
                    extract_layer(&layer_path, &extracted_path).await?;
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

    // Apply layers in order
    for (_, path) in extracted_paths {
        tracing::info!("Applying layer {}", path.display());
        super::copy(&path, &merged_dir, true).await?;
    }

    Ok(())
}

/// Extracts a tar.gz layer to the specified path.
/// Handles decompression and unpacking of OCI layer archives.
///
/// # Arguments
/// * `layer_path` - Path to the compressed layer archive
/// * `extract_path` - Directory to extract the layer into
///
/// # Notes
/// - Runs extraction in a blocking task to avoid blocking the async runtime
/// - Creates the extract directory if it doesn't exist
/// - Preserves file permissions and attributes during extraction
///
/// # Errors
/// Returns error if:
/// - Failed to open layer archive
/// - Failed to decompress gzip data
/// - Failed to extract tar contents
/// - Failed to create extract directory
async fn extract_layer(layer_path: &Path, extract_path: &Path) -> MonocoreResult<()> {
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(not(feature = "overlayfs"))]
#[cfg(test)]
mod tests {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};

    use tempfile::tempdir;

    use crate::oci::rootfs;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_oci_rootfs_merge_whiteout_handling() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;

        // Create test layers and get repo tag
        let repo_tag =
            helper::create_test_layers_with_whiteouts(&temp_dir.path().to_path_buf()).await?;

        // Setup merger
        let dest_dir = temp_dir.path().join("merged_whiteout_test");
        fs::create_dir_all(&dest_dir).await?;

        // Merge layers using the standard merge function
        merge(temp_dir.path(), dest_dir.clone(), &repo_tag).await?;

        // Verify regular whiteout
        let merged_dir = dest_dir.join(MERGED_SUBDIR);
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
        rootfs::remove(dest_dir).await?;

        Ok(())
    }

    #[test_log::test(tokio::test)]
    async fn test_oci_rootfs_merge_permissions_handling() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;

        // Create test layers and get repo tag
        let repo_tag =
            helper::create_test_layers_with_permissions(&temp_dir.path().to_path_buf()).await?;

        // Setup merger
        let dest_dir = temp_dir.path().join("merged_permissions_test");
        fs::create_dir_all(&dest_dir).await?;

        // Merge layers
        merge(temp_dir.path(), dest_dir.clone(), &repo_tag).await?;

        // Verify the merged results
        let merged_dir = dest_dir.join(MERGED_SUBDIR);

        // Helper function to verify permissions
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
        rootfs::remove(dest_dir).await?;

        Ok(())
    }

    mod helper {
        use std::{os::unix::fs::PermissionsExt, str::FromStr};

        use flate2::{write::GzEncoder, Compression};
        use nix::{sys::stat::Mode, unistd};
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
        /// │   ├── sha256_1111... (Layer 1 - Base)
        /// │   │   ├── file1.txt         ("original content")
        /// │   │   ├── file2.txt         ("keep this file")
        /// │   │   └── dir1/
        /// │   │       ├── inside1.txt   ("inside1")
        /// │   │       └── inside2.txt   ("inside2")
        /// │   │
        /// │   ├── sha256_2222... (Layer 2 - Regular Whiteout)
        /// │   │   ├── .wh.file1.txt    (removes file1.txt)
        /// │   │   └── file3.txt        ("new file")
        /// │   │
        /// │   └── sha256_3333... (Layer 3 - Opaque Whiteout)
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
                "sha256_1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
                "sha256_2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
                "sha256_3333333333333333333333333333333333333333333333333333333333333333"
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
                                Sha256Digest::from_str(digest_str.trim_start_matches("sha256_"))
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
        /// Layer 1 (Base Layer - sha256_4444...)
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
        /// Layer 2 (Symlinks Layer - sha256_5555...)
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
                "sha256_4444444444444444444444444444444444444444444444444444444444444444"
                    .to_string(),
                "sha256_5555555555555555555555555555555555555555555555555555555555555555"
                    .to_string(),
            ];

            // Layer 1: Base layer with various permission scenarios
            {
                // Create the extracted layer directory directly
                let layer_path = layers_dir
                    .join(&layer_digests[0])
                    .with_extension(EXTRACTED_LAYER_EXTENSION);
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
                    .with_extension(EXTRACTED_LAYER_EXTENSION);
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
                                Sha256Digest::from_str(digest_str.trim_start_matches("sha256_"))
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

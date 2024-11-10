use flate2::{write::GzEncoder, Compression};
use oci_spec::image::{DescriptorBuilder, ImageManifestBuilder, Sha256Digest};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, str::FromStr};
use tar::Builder;
use tempfile::tempdir;
use tokio::fs as tokio_fs;

use monocore::{
    oci::{
        distribution::{DockerRegistry, OciRegistryPull},
        overlayfs::OverlayFsMerger,
    },
    utils::OCI_SUBDIR,
};

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Docker images"]
async fn test_oci_merge_basic_merge() -> anyhow::Result<()> {
    // Create temporary directory for test
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull a small image with multiple layers (alpine:latest)
    registry
        .pull_image("library/alpine", Some("latest"))
        .await?;

    // Create merger with temporary destination
    let dest_dir = temp_dir.path().join("merged_test");
    fs::create_dir_all(&dest_dir)?;

    let merger = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir.clone());

    // Merge layers
    merger.merge("library_alpine__latest").await?;

    // Verify basic Alpine Linux directories exist
    let expected_dirs = vec!["bin", "etc", "home", "root", "usr", "var"];

    for dir in &expected_dirs {
        assert!(
            dest_dir.join("merged").join(dir).exists(),
            "Directory {} should exist",
            dir
        );
    }

    // Cleanup
    merger.unmount().await?;
    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Docker images"]
async fn test_oci_merge_whiteout_handling() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;

    // TODO: Debugging...
    println!("temp_dir: {}", temp_dir.path().display());

    // Create test layers and get repo tag
    let repo_tag = create_test_layers(&temp_dir.path().to_path_buf()).await?;

    // Setup merger
    let dest_dir = temp_dir.path().join("merged_whiteout_test");
    tokio_fs::create_dir_all(&dest_dir).await?;

    let merger = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir.clone());

    // Merge layers using the standard merge function
    merger.merge(&repo_tag).await?;

    // TODO: Debugging...
    tokio::time::sleep(tokio::time::Duration::from_secs(60 * 5)).await;

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
#[ignore = "requires pulling Docker images"]
async fn test_oci_merge_layer_permissions() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull nginx image which has specific file permissions
    registry.pull_image("library/nginx", Some("alpine")).await?;

    let dest_dir = temp_dir.path().join("merged_perms_test");
    fs::create_dir_all(&dest_dir)?;

    let merger = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir.clone());

    // Merge layers
    merger.merge("library_nginx__alpine").await?;

    // Verify nginx binary permissions
    let nginx_binary = dest_dir.join("merged/usr/sbin/nginx");
    let metadata = fs::metadata(&nginx_binary)?;
    let mode = metadata.permissions().mode();

    // nginx binary should be executable
    assert!(
        mode & 0o111 != 0,
        "nginx binary should have executable permissions"
    );

    // Cleanup
    merger.unmount().await?;
    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Docker images"]
async fn test_oci_merge_merge_cleanup() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull a small image
    registry
        .pull_image("library/alpine", Some("latest"))
        .await?;

    let dest_dir = temp_dir.path().join("merged_cleanup_test");
    fs::create_dir_all(&dest_dir)?;

    let merger = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir.clone());

    // Merge layers
    merger.merge("library_alpine__latest").await?;

    // Verify work directories are created
    assert!(dest_dir.join("work").exists());
    assert!(dest_dir.join("upper").exists());
    assert!(dest_dir.join("merged").exists());

    // Unmount and cleanup
    merger.unmount().await?;

    // Verify cleanup
    assert!(!dest_dir.join("work").exists());
    assert!(!dest_dir.join("upper").exists());
    assert!(!dest_dir.join("merged").exists());

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Docker images"]
async fn test_oci_merge_concurrent_merges() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull two different images
    let pull_tasks = tokio::join!(
        registry.pull_image("library/alpine", Some("latest")),
        registry.pull_image("library/busybox", Some("latest"))
    );
    pull_tasks.0?;
    pull_tasks.1?;

    // Create two separate merge destinations
    let dest_dir1 = temp_dir.path().join("merged_concurrent_1");
    let dest_dir2 = temp_dir.path().join("merged_concurrent_2");
    fs::create_dir_all(&dest_dir1)?;
    fs::create_dir_all(&dest_dir2)?;

    let merger1 = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir1.clone());
    let merger2 = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir2.clone());

    // Merge concurrently
    let merge_results = tokio::join!(
        merger1.merge("library_alpine__latest"),
        merger2.merge("library_busybox__latest")
    );

    // Check results
    merge_results.0?;
    merge_results.1?;

    // Verify both merges succeeded
    assert!(dest_dir1.join("merged/bin").exists());
    assert!(dest_dir2.join("merged/bin").exists());

    // Cleanup
    let cleanup_results = tokio::join!(merger1.unmount(), merger2.unmount());
    cleanup_results.0?;
    cleanup_results.1?;

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Docker images"]
async fn test_oci_merge_error_handling() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;

    // Try to merge non-existent image
    let dest_dir = temp_dir.path().join("merged_error_test");
    fs::create_dir_all(&dest_dir)?;

    let merger = OverlayFsMerger::new(temp_dir.path().join(OCI_SUBDIR), dest_dir.clone());

    // This should fail because no image was pulled
    let result = merger.merge("nonexistent_image").await;
    assert!(result.is_err());

    // Verify cleanup happened despite error
    assert!(!dest_dir.join("work").exists());
    assert!(!dest_dir.join("upper").exists());
    assert!(!dest_dir.join("merged").exists());

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

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
async fn create_test_layers(base_dir: &PathBuf) -> anyhow::Result<String> {
    use monocore::utils::{OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME, OCI_REPO_SUBDIR};
    use serde_json::to_string_pretty;

    // Create OCI directory structure
    let oci_dir = base_dir.join(OCI_SUBDIR);
    let layers_dir = oci_dir.join(OCI_LAYER_SUBDIR);
    let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join("test_layers");

    for dir in [&layers_dir, &repo_dir] {
        tokio_fs::create_dir_all(dir).await?;
    }

    // Create layer directories and their content
    let layer_digests = vec![
        "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        "sha256:3333333333333333333333333333333333333333333333333333333333333333".to_string(),
    ];

    // Create temporary directory for layer contents
    let temp_dir = tempdir()?;

    // Layer 1: Base files
    {
        let layer_contents = temp_dir.path().join("layer1");
        tokio_fs::create_dir_all(&layer_contents).await?;
        tokio_fs::write(layer_contents.join("file1.txt"), "original content").await?;
        tokio_fs::write(layer_contents.join("file2.txt"), "keep this file").await?;
        tokio_fs::create_dir(layer_contents.join("dir1")).await?;
        tokio_fs::write(layer_contents.join("dir1/inside1.txt"), "inside1").await?;
        tokio_fs::write(layer_contents.join("dir1/inside2.txt"), "inside2").await?;

        // Create tar.gz for layer 1
        let layer_file = fs::File::create(layers_dir.join(&layer_digests[0]))?;
        let encoder = GzEncoder::new(layer_file, Compression::default());
        let mut tar = Builder::new(encoder);
        tar.append_dir_all(".", layer_contents)?;
        tar.finish()?;
    }

    // Layer 2: Regular whiteout
    {
        let layer_contents = temp_dir.path().join("layer2");
        tokio_fs::create_dir_all(&layer_contents).await?;
        tokio_fs::write(layer_contents.join(".wh.file1.txt"), "").await?;
        tokio_fs::write(layer_contents.join("file3.txt"), "new file").await?;

        // Create tar.gz for layer 2
        let layer_file = fs::File::create(layers_dir.join(&layer_digests[1]))?;
        let encoder = GzEncoder::new(layer_file, Compression::default());
        let mut tar = Builder::new(encoder);
        tar.append_dir_all(".", layer_contents)?;
        tar.finish()?;
    }

    // Layer 3: Opaque whiteout
    {
        let layer_contents = temp_dir.path().join("layer3");
        tokio_fs::create_dir_all(&layer_contents).await?;
        tokio_fs::create_dir(layer_contents.join("dir1")).await?;
        tokio_fs::write(layer_contents.join("dir1/.wh..wh..opq"), "").await?;
        tokio_fs::write(layer_contents.join("dir1/new_file.txt"), "new content").await?;

        // Create tar.gz for layer 3
        let layer_file = fs::File::create(layers_dir.join(&layer_digests[2]))?;
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
                    let digest = Sha256Digest::from_str(digest_str.trim_start_matches("sha256:"))
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
    tokio_fs::write(&manifest_path, to_string_pretty(&manifest)?).await?;

    Ok("test_layers".to_string())
}

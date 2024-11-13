use std::path::Path;

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use crate::oci::rootfs;
use crate::{
    oci::distribution::{DockerRegistry, OciRegistryPull},
    MonocoreResult,
};

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use tokio::fs;

use super::OCI_REPO_SUBDIR;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Pulls an OCI image from a Docker registry and stores it in the specified directory.
/// If the image already exists in the artifacts directory, the pull is skipped.
///
/// ## Arguments
/// * `oci_dir` - Directory where artifacts of the pulled image will be stored
/// * `image_ref` - Image reference (e.g., "library/alpine:latest")
///
/// ## Example
/// ```rust,no_run
/// use monocore::utils;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     utils::pull_docker_image("/path/to/oci", "library/alpine:latest").await?;
///     Ok(())
/// }
/// ```
pub async fn pull_docker_image(oci_dir: impl AsRef<Path>, image_ref: &str) -> MonocoreResult<()> {
    let oci_dir = oci_dir.as_ref();
    let (repository, tag, repo_tag) = super::parse_image_ref(image_ref)?;

    // Check if image already exists
    let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join(&repo_tag);

    if repo_dir.exists() {
        tracing::info!(
            "Image {image_ref} already exists in {}, skipping pull",
            oci_dir.display()
        );
        return Ok(());
    }

    let registry = DockerRegistry::with_oci_dir(oci_dir.into());
    registry.pull_image(repository, Some(tag)).await
}

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
/// Merges OCI image layers into a single rootfs directory.
/// If the destination directory already exists, the merge is skipped.
///
/// ## Arguments
/// * `oci_dir` - Directory containing the OCI artifacts of the pulled image
/// * `dest_dir` - Directory where the merged rootfs will be created
/// * `image_ref` - Image reference (e.g., "library/alpine:latest")
///
/// ## Example
/// ```rust,no_run
/// use monocore::utils;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     utils::merge_image_layers(
///         "/path/to/oci",
///         "/path/to/rootfs",
///         "library/alpine:latest"
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn merge_image_layers(
    oci_dir: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
    image_ref: &str,
) -> MonocoreResult<()> {
    let oci_dir = oci_dir.as_ref();
    let dest_dir = dest_dir.as_ref();

    // Check if destination already exists
    if dest_dir.exists() {
        tracing::info!(
            "Rootfs already exists at {}, skipping merge",
            dest_dir.display()
        );
        return Ok(());
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = dest_dir.parent() {
        fs::create_dir_all(parent).await?;
    }

    let repo_tag = super::parse_image_ref(image_ref)?.2;

    rootfs::merge(oci_dir, dest_dir, &repo_tag).await
}

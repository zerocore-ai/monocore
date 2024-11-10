use crate::{
    oci::{
        distribution::{DockerRegistry, OciRegistryPull},
        overlayfs::OverlayFsMerger,
    },
    utils::conversion::sanitize_repo_name,
    MonocoreResult,
};
use std::path::PathBuf;
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
/// * `repository` - Repository name (e.g., "library/alpine")
/// * `tag` - Image tag (e.g., "latest")
///
/// ## Example
/// ```rust,no_run
/// use monocore::utils;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     utils::pull_docker_image("/path/to/oci", "library/alpine", "latest").await?;
///     Ok(())
/// }
/// ```
pub async fn pull_docker_image(
    oci_dir: impl Into<PathBuf>,
    repository: &str,
    tag: &str,
) -> MonocoreResult<()> {
    let oci_dir = oci_dir.into();
    let repo_tag = format!(
        "{}__{}",
        sanitize_repo_name(repository),
        sanitize_repo_name(tag)
    );

    // Check if image already exists
    let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join(&repo_tag);

    if repo_dir.exists() {
        tracing::info!(
            "Image {repository}:{tag} already exists in {}, skipping pull",
            oci_dir.display()
        );
        return Ok(());
    }

    let registry = DockerRegistry::with_oci_dir(oci_dir);
    registry.pull_image(repository, Some(tag)).await
}

/// Merges OCI image layers into a single rootfs directory.
/// If the destination directory already exists, the merge is skipped.
///
/// ## Arguments
/// * `oci_dir` - Directory containing the OCI artifacts of the pulled image
/// * `dest_dir` - Directory where the merged rootfs will be created
/// * `repository` - Repository name (e.g., "library/alpine")
/// * `tag` - Image tag (e.g., "latest")
///
/// ## Example
/// ```rust,no_run
/// use monocore::utils;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     utils::merge_image_layers(
///         "/path/to/artifacts",
///         "/path/to/rootfs",
///         "library/alpine",
///         "latest"
///     ).await?;
///     Ok(())
/// }
/// ```
pub async fn merge_image_layers(
    oci_dir: impl Into<PathBuf>,
    dest_dir: impl Into<PathBuf>,
    repository: &str,
    tag: &str,
) -> MonocoreResult<()> {
    let dest_dir = dest_dir.into();

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

    let repo_tag = format!(
        "{}__{}",
        sanitize_repo_name(repository),
        sanitize_repo_name(tag)
    );

    let merger = OverlayFsMerger::new(oci_dir, dest_dir);
    merger.merge(&repo_tag).await
}

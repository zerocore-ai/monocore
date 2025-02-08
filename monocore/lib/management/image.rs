use crate::{
    oci::{DockerRegistry, OciRegistryPull, Reference},
    MonocoreError, MonocoreResult,
};
use tempfile::tempdir;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The domain name for the Sandboxes.io registry.
const SANDBOXES_REGISTRY: &str = "sandboxes.io";

/// The domain name for the Docker registry.
const DOCKER_REGISTRY: &str = "docker.io";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Pulls an image or image group from a supported registry (Docker or Sandboxes.io).
///
/// This function handles pulling container images from different registries based on the provided
/// parameters. It supports both single image pulls and image group pulls (for Sandboxes.io registry only).
///
/// ## Arguments
///
/// * `name` - The reference to the image or image group to pull
/// * `image` - If true, indicates that a single image should be pulled
/// * `image_group` - If true, indicates that an image group should be pulled (Sandboxes.io only)
///
/// ## Errors
///
/// Returns an error in the following cases:
/// * Both `image` and `image_group` are true (invalid combination)
/// * Image group pull is requested for a non-Sandboxes.io registry
/// * Unsupported registry is specified
/// * Registry-specific pull operations fail
///
/// # Examples
///
/// ```no_run
/// use crate::management::image::pull_image;
/// use crate::oci::Reference;
///
/// // Pull a single image from Docker registry
/// pull_image("docker.io/library/ubuntu:latest".parse().unwrap(), true, false).await?;
///
/// // Pull an image group from Sandboxes.io registry
/// pull_image("sandboxes.io/mygroup:latest".parse().unwrap(), false, true).await?;
/// ```
pub async fn pull_image(name: Reference, image: bool, image_group: bool) -> MonocoreResult<()> {
    // Both cannot be true
    if image && image_group {
        return Err(MonocoreError::InvalidArgument(
            "both image and image_group cannot be true".to_string(),
        ));
    }

    if image_group {
        // Only sandboxes registry supports image group pulls.
        let registry = name.to_string().split('/').next().unwrap_or("").to_string();

        if registry != SANDBOXES_REGISTRY {
            return Err(MonocoreError::InvalidArgument(format!(
                "Image group pull is only supported for sandboxes registry, got: {}",
                registry
            )));
        }

        // In image group mode, no fallback is applied
        return pull_sandboxes_registry_image_group(name.clone()).await;
    }

    // Single image pull mode (default if both flags are false, or if image is true)
    let registry = name.to_string().split('/').next().unwrap_or("").to_string();
    if registry == DOCKER_REGISTRY {
        pull_docker_registry_image(name).await
    } else if registry == SANDBOXES_REGISTRY {
        match pull_sandboxes_registry_image(name.clone()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!("Sandboxes registry image pull failed: {}. Falling back to DockerRegistry pull.", e);
                pull_docker_registry_image(name).await
            }
        }
    } else {
        Err(MonocoreError::InvalidArgument(format!(
            "Unsupported registry: {}",
            registry
        )))
    }
}

/// Pulls a single image from the Docker registry.
///
/// ## Arguments
///
/// * `image` - The reference to the Docker image to pull
///
/// ## Errors
///
/// Returns an error if:
/// * Failed to create temporary directories
/// * Failed to initialize Docker registry client
/// * Failed to pull the image from Docker registry
pub async fn pull_docker_registry_image(image: Reference) -> MonocoreResult<()> {
    // Create a temporary directory for downloading layers
    let temp_download_dir = tempdir().map_err(|e| MonocoreError::NotImplemented(e.to_string()))?;

    // Create a temporary directory for the OCI database
    let temp_db_dir = tempdir().map_err(|e| MonocoreError::NotImplemented(e.to_string()))?;
    let db_path = temp_db_dir.path().join("temp.db");

    let docker_registry =
        DockerRegistry::new(temp_download_dir.path().to_path_buf(), db_path).await?;

    docker_registry
        .pull_image(image.get_repository(), image.get_selector().clone())
        .await
}

/// Pulls a single image from the Sandboxes.io registry.
pub async fn pull_sandboxes_registry_image(_image: Reference) -> MonocoreResult<()> {
    return Err(MonocoreError::NotImplemented(
        "Sandboxes registry image pull is not implemented".to_string(),
    ));
}

/// Pulls an image group from the Sandboxes.io registry.
pub async fn pull_sandboxes_registry_image_group(_group: Reference) -> MonocoreResult<()> {
    return Err(MonocoreError::NotImplemented(
        "Sandboxes registry image group pull is not implemented".to_string(),
    ));
}

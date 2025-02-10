use crate::management::db::{self, OCI_DB_MIGRATOR};
use crate::utils::{env::get_monocore_home_path, path::OCI_DB_FILENAME};
use crate::{
    oci::{DockerRegistry, OciRegistryPull, Reference},
    MonocoreError, MonocoreResult,
};
use futures::future;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use tokio::{fs, process::Command};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The domain name for the Sandboxes.io registry.
const SANDBOXES_REGISTRY: &str = "sandboxes.io";

/// The domain name for the Docker registry.
const DOCKER_REGISTRY: &str = "docker.io";

/// The suffix added to extracted layer directories
const EXTRACTED_LAYER_SUFFIX: &str = "extracted";

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
        return pull_sandboxes_registry_image_group(&name).await;
    }

    // Single image pull mode (default if both flags are false, or if image is true)
    let registry = name.to_string().split('/').next().unwrap_or("").to_string();
    if registry == DOCKER_REGISTRY {
        pull_docker_registry_image(&name).await
    } else if registry == SANDBOXES_REGISTRY {
        match pull_sandboxes_registry_image(&name).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!(
                    "Sandboxes registry image pull failed: {}. Falling back to DockerRegistry pull.",
                    e
                );
                // Create a new reference with docker.io registry for fallback
                let mut docker_ref = name.clone();
                docker_ref.set_registry(DOCKER_REGISTRY.to_string());
                pull_docker_registry_image(&docker_ref).await
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
pub async fn pull_docker_registry_image(image: &Reference) -> MonocoreResult<()> {
    // Create a temporary directory for downloading layers
    let temp_download_dir = tempdir()
        .map_err(|e| MonocoreError::NotImplemented(e.to_string()))?
        .into_path();
    // let temp_download_dir = get_monocore_home_path().join("tmp"); // TODO: Remove. Placeholder for debugging.

    // Get the global OCI database path
    let db_path = get_monocore_home_path().join(OCI_DB_FILENAME);

    let docker_registry = DockerRegistry::new(&temp_download_dir, db_path.clone()).await?;

    // Get or create a connection pool to the database
    let pool = db::get_or_create_db_pool(&db_path, &OCI_DB_MIGRATOR).await?;

    // Check if the image already exists in the database
    tracing::info!("Checking if image {} already exists in database", image);
    if db::image_exists(&pool, &image.to_string()).await? {
        tracing::info!("Image {} already exists in database, skipping pull", image);
        return Ok(());
    }

    docker_registry
        .pull_image(image.get_repository(), image.get_selector().clone())
        .await?;

    // Find and extract layers in parallel
    let layer_paths = collect_layer_files(&temp_download_dir).await?;

    let extraction_futures: Vec<_> = layer_paths
        .into_iter()
        .map(|path| async move { extract_layer(path).await })
        .collect();

    // Wait for all extractions to complete
    for result in future::join_all(extraction_futures).await {
        result?;
    }

    //

    Ok(())
}

/// Pulls a single image from the Sandboxes.io registry.
pub async fn pull_sandboxes_registry_image(_image: &Reference) -> MonocoreResult<()> {
    return Err(MonocoreError::NotImplemented(
        "Sandboxes registry image pull is not implemented".to_string(),
    ));
}

/// Pulls an image group from the Sandboxes.io registry.
pub async fn pull_sandboxes_registry_image_group(_group: &Reference) -> MonocoreResult<()> {
    return Err(MonocoreError::NotImplemented(
        "Sandboxes registry image group pull is not implemented".to_string(),
    ));
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Extracts a layer from the downloaded tar.gz file into an extracted directory.
/// The extracted directory will be named as <layer-name>.extracted
async fn extract_layer(layer_path: impl AsRef<std::path::Path>) -> MonocoreResult<()> {
    let layer_path = layer_path.as_ref();
    let file_name = layer_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| MonocoreError::LayerHandling {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "Invalid layer file name"),
            layer: layer_path.display().to_string(),
        })?;

    let parent_dir = layer_path
        .parent()
        .ok_or_else(|| MonocoreError::LayerHandling {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "Parent directory not found"),
            layer: file_name.to_string(),
        })?;

    // Create the extraction directory with name <layer-name>.extracted
    let extract_dir = parent_dir.join(format!("{}.{}", file_name, EXTRACTED_LAYER_SUFFIX));
    fs::create_dir_all(&extract_dir)
        .await
        .map_err(|e| MonocoreError::LayerHandling {
            source: e,
            layer: file_name.to_string(),
        })?;

    tracing::info!(
        "Extracting layer {} to {}",
        file_name,
        extract_dir.display()
    );

    // Use tar command to extract the layer
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(layer_path)
        .arg("-C")
        .arg(&extract_dir)
        .output()
        .await
        .map_err(|e| MonocoreError::LayerHandling {
            source: e,
            layer: file_name.to_string(),
        })?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(MonocoreError::LayerExtraction(format!(
            "Failed to extract layer {}: {}",
            file_name, error_msg
        )));
    }

    tracing::info!(
        "Successfully extracted layer {} to {}",
        file_name,
        extract_dir.display()
    );
    Ok(())
}

/// Collects all layer files in the given directory that start with "sha256:".
async fn collect_layer_files(dir: impl AsRef<Path>) -> MonocoreResult<Vec<PathBuf>> {
    let mut layer_paths = Vec::new();
    let mut read_dir = fs::read_dir(dir).await?;

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("sha256:") {
                    layer_paths.push(path.clone());
                }
            }
        }
    }

    tracing::info!("Found {} layers to extract", layer_paths.len());
    Ok(layer_paths)
}

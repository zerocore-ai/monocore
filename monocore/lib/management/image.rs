//! Container image management for Monocore.
//!
//! This module provides functionality for managing container images from various
//! registries. It supports pulling images from Docker and Sandboxes.io registries,
//! handling image layers, and managing the local image cache.

use crate::{
    management::db::{self, OCI_DB_MIGRATOR},
    oci::{DockerRegistry, OciRegistryPull, Reference},
    utils::{
        env::get_monocore_home_path,
        path::{LAYERS_SUBDIR, OCI_DB_FILENAME},
        EXTRACTED_LAYER_SUFFIX,
    },
    MonocoreError, MonocoreResult,
};
use futures::future;
use sqlx::{Pool, Sqlite};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use tokio::{fs, process::Command};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

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
/// * `layer_path` - The path to store the layer files
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
/// use monocore::management::pull_image;
/// use monocore::oci::Reference;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// // Pull a single image from Docker registry
/// pull_image("docker.io/library/ubuntu:latest".parse().unwrap(), true, false, None).await?;
///
/// // Pull an image from Sandboxes.io registry
/// pull_image("myimage".parse().unwrap(), false, false, None).await?;
///
/// // Pull an image group from Sandboxes.io registry
/// pull_image("sandboxes.io/mygroup:latest".parse().unwrap(), false, true, None).await?;
///
/// // Pull an image from Docker registry and store the layers in a custom directory
/// pull_image("docker.io/library/ubuntu:latest".parse().unwrap(), true, false, Some(PathBuf::from("/custom/path"))).await?;
/// # Ok(())
/// # }
/// ```
pub async fn pull_image(
    name: Reference,
    image: bool,
    image_group: bool,
    layer_path: Option<PathBuf>,
) -> MonocoreResult<()> {
    // Both cannot be true
    if image && image_group {
        return Err(MonocoreError::InvalidArgument(
            "both image and image_group cannot be true".to_string(),
        ));
    }

    if image_group {
        return Err(MonocoreError::InvalidArgument(
            "image group pull is currently not supported".to_string(),
        ));
    }

    // Single image pull mode (default if both flags are false, or if image is true)
    let registry = name.to_string().split('/').next().unwrap_or("").to_string();
    let temp_download_dir = tempdir()?.into_path();
    if registry == DOCKER_REGISTRY {
        pull_docker_registry_image(&name, &temp_download_dir, layer_path).await
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
/// * `download_dir` - The directory to download the image layers to
/// * `layer_path` - Optional custom path to store layers
///
/// ## Errors
///
/// Returns an error if:
/// * Failed to create temporary directories
/// * Failed to initialize Docker registry client
/// * Failed to pull the image from Docker registry
pub async fn pull_docker_registry_image(
    image: &Reference,
    download_dir: impl AsRef<Path>,
    layer_path: Option<PathBuf>,
) -> MonocoreResult<()> {
    let download_dir = download_dir.as_ref();
    let monocore_home_path = get_monocore_home_path();
    let db_path = monocore_home_path.join(OCI_DB_FILENAME);

    // Use custom layer_path if specified, otherwise use default monocore layers directory
    let layers_dir = match layer_path {
        Some(path) => path,
        None => monocore_home_path.join(LAYERS_SUBDIR),
    };

    // Create layers directory if it doesn't exist
    fs::create_dir_all(&layers_dir).await?;

    let docker_registry = DockerRegistry::new(download_dir, &db_path).await?;

    // Get or create a connection pool to the database
    let pool = db::get_or_create_db_pool(&db_path, &OCI_DB_MIGRATOR).await?;

    // Check if we need to pull the image
    if check_image_layers(&pool, image, &layers_dir).await? {
        tracing::info!("image {} and all its layers exist, skipping pull", image);
        return Ok(());
    }

    docker_registry
        .pull_image(image.get_repository(), image.get_selector().clone())
        .await?;

    // Find and extract layers in parallel
    let layer_paths = collect_layer_files(download_dir).await?;

    let extraction_futures: Vec<_> = layer_paths
        .into_iter()
        .map(|path| {
            let layers_dir = layers_dir.clone();
            async move { extract_layer(path, &layers_dir).await }
        })
        .collect();

    // Wait for all extractions to complete
    for result in future::join_all(extraction_futures).await {
        result?;
    }

    Ok(())
}

/// Pulls a single image from the Sandboxes.io registry.
///
/// ## Arguments
///
/// * `image` - The reference to the Sandboxes.io image to pull
/// ## Errors
///
/// Returns an error if:
/// * Sandboxes registry image pull is not implemented
pub async fn pull_sandboxes_registry_image(_image: &Reference) -> MonocoreResult<()> {
    return Err(MonocoreError::NotImplemented(
        "sandboxes registry image pull is not implemented".to_string(),
    ));
}

/// Pulls an image group from the Sandboxes.io registry.
///
/// ## Arguments
///
/// * `group` - The reference to the image group to pull
/// ## Errors
///
/// Returns an error if:
/// * Sandboxes registry image group pull is not implemented
pub async fn pull_sandboxes_registry_image_group(_group: &Reference) -> MonocoreResult<()> {
    return Err(MonocoreError::NotImplemented(
        "Sandboxes registry image group pull is not implemented".to_string(),
    ));
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Checks if all layers for an image exist in both the database and the layers directory.
///
/// ## Arguments
///
/// * `pool` - The database connection pool
/// * `image` - The reference to the image to check
/// * `layers_dir` - The directory where layers should be stored
///
/// ## Returns
///
/// Returns Ok(true) if all layers exist and are valid, Ok(false) if any layers are missing
/// or invalid. Any errors during the check process will return Ok(false) with a warning log.
async fn check_image_layers(
    pool: &Pool<Sqlite>,
    image: &Reference,
    layers_dir: impl AsRef<Path>,
) -> MonocoreResult<bool> {
    let layers_dir = layers_dir.as_ref();

    // Check if the image exists in the database
    match db::image_exists(pool, &image.to_string()).await {
        Ok(true) => {
            // Image exists, check if all layers are present
            match db::get_image_layers(pool, &image.to_string()).await {
                Ok(layers) => {
                    for (digest, _, _) in layers {
                        // Check if layer exists in the layers directory
                        let layer_path =
                            layers_dir.join(format!("{}.{}", digest, EXTRACTED_LAYER_SUFFIX));
                        if !layer_path.exists() {
                            tracing::warn!("layer {} not found in layers directory", digest);
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                Err(e) => {
                    tracing::warn!("error checking layers: {}, will pull image", e);
                    Ok(false)
                }
            }
        }
        Ok(false) => Ok(false),
        Err(e) => {
            tracing::warn!("error checking image existence: {}, will pull image", e);
            Ok(false)
        }
    }
}

/// Extracts a layer from the downloaded tar.gz file into an extracted directory.
/// The extracted directory will be named as <layer-name>.extracted
async fn extract_layer(
    layer_path: impl AsRef<std::path::Path>,
    extract_base_dir: impl AsRef<Path>,
) -> MonocoreResult<()> {
    let layer_path = layer_path.as_ref();
    let file_name = layer_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| MonocoreError::LayerHandling {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "invalid layer file name"),
            layer: layer_path.display().to_string(),
        })?;

    // Create the extraction directory with name <layer-name>.extracted
    let extract_dir = extract_base_dir
        .as_ref()
        .join(format!("{}.{}", file_name, EXTRACTED_LAYER_SUFFIX));

    // Check if the layer is already extracted
    if extract_dir.exists() {
        // Check if the directory has content (not empty)
        let mut read_dir =
            fs::read_dir(&extract_dir)
                .await
                .map_err(|e| MonocoreError::LayerHandling {
                    source: e,
                    layer: file_name.to_string(),
                })?;

        if read_dir.next_entry().await?.is_some() {
            tracing::info!(
                "layer {} already extracted at {}, skipping extraction",
                file_name,
                extract_dir.display()
            );
            return Ok(());
        }
    }

    fs::create_dir_all(&extract_dir)
        .await
        .map_err(|e| MonocoreError::LayerHandling {
            source: e,
            layer: file_name.to_string(),
        })?;

    tracing::info!(
        "extracting layer {} to {}",
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
        "successfully extracted layer {} to {}",
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

    tracing::info!("found {} layers to extract", layer_paths.len());
    Ok(layer_paths)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test_log::test(tokio::test)]
    #[ignore = "makes network requests to Docker registry to pull an image"]
    async fn test_image_pull_docker_registry_image() -> MonocoreResult<()> {
        // Create temporary directories for test
        let temp_dir = TempDir::new()?;
        let monocore_home = temp_dir.path().join("monocore_home");
        let download_dir = temp_dir.path().join("download");
        fs::create_dir_all(&monocore_home).await?;
        fs::create_dir_all(&download_dir).await?;

        // Set up test environment
        std::env::set_var("MONOCORE_HOME", monocore_home.to_str().unwrap());

        // Create test image reference (using a small image for faster tests)
        let image_ref: Reference = "docker.io/library/nginx:stable-alpine".parse().unwrap();

        // Call the function under test
        pull_docker_registry_image(&image_ref, &download_dir, None).await?;

        // Initialize database connection for verification
        let db_path = monocore_home.join(OCI_DB_FILENAME);
        let pool = db::get_or_create_db_pool(&db_path, &OCI_DB_MIGRATOR).await?;

        // Verify image exists in database
        let image_exists = db::image_exists(&pool, &image_ref.to_string()).await?;
        assert!(image_exists, "Image should exist in database");

        // Verify layers directory exists and contains extracted layers
        let layers_dir = monocore_home.join(LAYERS_SUBDIR);
        assert!(layers_dir.exists(), "Layers directory should exist");

        // Verify extracted layer directories exist
        let mut entries = fs::read_dir(&layers_dir).await?;
        let mut found_extracted_layers = false;
        while let Some(entry) = entries.next_entry().await? {
            if entry
                .file_name()
                .to_string_lossy()
                .ends_with(EXTRACTED_LAYER_SUFFIX)
            {
                found_extracted_layers = true;
                assert!(
                    entry.path().is_dir(),
                    "Extracted layer path should be a directory"
                );
            }
        }
        assert!(
            found_extracted_layers,
            "Should have found extracted layer directories"
        );

        // Verify nginx files exist in the extracted layers
        helper::verify_nginx_files(&layers_dir).await?;

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use super::*;

    /// Helper function to verify that all expected nginx files exist in the extracted layers
    pub(super) async fn verify_nginx_files(layers_dir: impl AsRef<Path>) -> MonocoreResult<()> {
        let mut found_nginx_conf = false;
        let mut found_default_conf = false;
        let mut found_nginx_binary = false;

        // Check each extracted layer directory for nginx files
        let mut entries = fs::read_dir(layers_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry
                .file_name()
                .to_string_lossy()
                .ends_with(EXTRACTED_LAYER_SUFFIX)
            {
                continue;
            }

            let layer_path = entry.path();
            tracing::info!("Checking layer: {}", layer_path.display());

            // Check for nginx.conf
            let nginx_conf = layer_path.join("etc").join("nginx").join("nginx.conf");
            if nginx_conf.exists() {
                found_nginx_conf = true;
                tracing::info!("Found nginx.conf at {}", nginx_conf.display());
            }

            // Check for default.conf
            let default_conf = layer_path
                .join("etc")
                .join("nginx")
                .join("conf.d")
                .join("default.conf");
            if default_conf.exists() {
                found_default_conf = true;
                tracing::info!("Found default.conf at {}", default_conf.display());
            }

            // Check for nginx binary
            let nginx_binary = layer_path.join("usr").join("sbin").join("nginx");
            if nginx_binary.exists() {
                found_nginx_binary = true;
                tracing::info!("Found nginx binary at {}", nginx_binary.display());
            }

            // If we found all files, we can stop checking
            if found_nginx_conf && found_default_conf && found_nginx_binary {
                break;
            }
        }

        // Assert that we found all the expected files
        assert!(
            found_nginx_conf,
            "nginx.conf should exist in one of the layers"
        );
        assert!(
            found_default_conf,
            "default.conf should exist in one of the layers"
        );
        assert!(
            found_nginx_binary,
            "nginx binary should exist in one of the layers"
        );

        Ok(())
    }
}

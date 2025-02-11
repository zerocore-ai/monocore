use crate::management::db::{self, OCI_DB_MIGRATOR};
use crate::management::rootfs;
use crate::utils::BLOCKS_SUBDIR;
use crate::utils::{env::get_monocore_home_path, path::OCI_DB_FILENAME};
use crate::{
    oci::{DockerRegistry, OciRegistryPull, Reference},
    MonocoreError, MonocoreResult,
};
use futures::future;
use monofs::filesystem::Dir;
use monofs::store::FlatFsStore;
use monoutils_store::ipld::cid::Cid;
use monoutils_store::{IpldStore, Storable};
use sqlx::SqlitePool;
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
/// use monocore::management::pull_image;
/// use monocore::oci::Reference;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// // Pull a single image from Docker registry
/// pull_image("docker.io/library/ubuntu:latest".parse().unwrap(), true, false).await?;
///
/// // Pull an image from Sandboxes.io registry
/// pull_image("myimage".parse().unwrap(), false, false).await?;
///
/// // Pull an image group from Sandboxes.io registry
/// pull_image("sandboxes.io/mygroup:latest".parse().unwrap(), false, true).await?;
/// # Ok(())
/// # }
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
    let temp_download_dir = tempdir()?.into_path();
    if registry == DOCKER_REGISTRY {
        pull_docker_registry_image(&name, &temp_download_dir).await
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
                pull_docker_registry_image(&docker_ref, &temp_download_dir).await
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
/// * `download_dir` - The directory to download the image layers to
/// ## Errors
///
/// Returns an error if:
/// * Failed to create temporary directories
/// * Failed to initialize Docker registry client
/// * Failed to pull the image from Docker registry
pub async fn pull_docker_registry_image(
    image: &Reference,
    download_dir: impl AsRef<Path>,
) -> MonocoreResult<()> {
    let download_dir = download_dir.as_ref();
    let monocore_home_path = get_monocore_home_path();
    let db_path = monocore_home_path.join(OCI_DB_FILENAME);

    let docker_registry = DockerRegistry::new(download_dir, &db_path).await?;

    // Get or create a connection pool to the database
    let pool = db::get_or_create_db_pool(&db_path, &OCI_DB_MIGRATOR).await?;

    // Check if the image already exists and is complete in the database
    tracing::info!("Checking if image {} already exists in database", image);
    if db::image_complete(&pool, &image.to_string()).await? {
        tracing::info!(
            "Image {} already exists and is complete in database, skipping pull",
            image
        );
        return Ok(());
    }

    docker_registry
        .pull_image(image.get_repository(), image.get_selector().clone())
        .await?;

    // Find and extract layers in parallel
    let layer_paths = collect_layer_files(download_dir).await?;

    let extraction_futures: Vec<_> = layer_paths
        .into_iter()
        .map(|path| async move { extract_layer(path).await })
        .collect();

    // Wait for all extractions to complete
    for result in future::join_all(extraction_futures).await {
        result?;
    }

    // Create monofs layers from extracted OCI layers and store their CIDs
    let store_path = monocore_home_path.join(BLOCKS_SUBDIR);
    fs::create_dir_all(&store_path).await?;
    let store = FlatFsStore::new(store_path);
    let _ = create_monofs_layers_from_extracted(&pool, download_dir, store.clone()).await?;

    // Get and merge the layers into a single monofs layer
    let layer_dirs = get_ordered_layer_dirs(&pool, &image.to_string(), store.clone()).await?;
    let (root_cid, _) = rootfs::merge_oci_based_monofs_layers(layer_dirs, store.clone()).await?;

    // Update the image's head_cid with the merged layer's root CID
    db::update_image_head_cid(&pool, &image.to_string(), &root_cid.to_string()).await?;

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
        "Sandboxes registry image pull is not implemented".to_string(),
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

/// Creates monofs layers from extracted OCI layers and stores their CIDs in the database.
///
/// This function:
/// 1. Creates a monofs layer from each extracted OCI layer directory
/// 2. Stores the root CID of each layer in the database
/// 3. Returns a vector of tuples containing the layer digest, root CID, and monofs directory
async fn create_monofs_layers_from_extracted<S>(
    pool: &SqlitePool,
    download_dir: impl AsRef<Path>,
    store: S,
) -> MonocoreResult<Vec<(String, String, Dir<S>)>>
where
    S: IpldStore + Clone + Send + Sync + 'static,
{
    let mut monofs_layers = Vec::new();
    let mut read_dir = fs::read_dir(download_dir).await?;

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
                MonocoreError::LayerHandling {
                    source: std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Invalid layer directory name",
                    ),
                    layer: path.display().to_string(),
                }
            })?;

            // Skip if not an extracted layer directory
            if !file_name.ends_with(EXTRACTED_LAYER_SUFFIX) {
                continue;
            }

            // Get the original layer name (without .extracted suffix)
            let layer_name = file_name
                .strip_suffix(&format!(".{}", EXTRACTED_LAYER_SUFFIX))
                .ok_or_else(|| MonocoreError::LayerHandling {
                    source: std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Invalid layer name format",
                    ),
                    layer: file_name.to_string(),
                })?;

            // Create monofs layer from the extracted directory
            tracing::info!("Creating monofs layer from {}", path.display());
            let (root_cid, root_dir) =
                rootfs::create_monofs_from_oci_layer(&path, store.clone()).await?;

            // Update the layer's head_cid in the database
            db::update_layer_head_cid(pool, layer_name, &root_cid.to_string()).await?;

            monofs_layers.push((layer_name.to_string(), root_cid.to_string(), root_dir));
        }
    }

    Ok(monofs_layers)
}

/// Gathers all layer directories for an image in base-to-top order.
///
/// This function:
/// 1. Gets all layer CIDs from the database in the correct order
/// 2. Verifies that all required layers have been processed
/// 3. Loads each layer's directory from its CID
/// 4. Returns a vector of layer directories in base-to-top order
///
/// ## Arguments
///
/// * `pool` - The database connection pool
/// * `reference` - The reference of the image to get layers for
/// * `store` - The IPLD store to load directories from
async fn get_ordered_layer_dirs<S>(
    pool: &SqlitePool,
    reference: &str,
    store: S,
) -> MonocoreResult<Vec<Dir<S>>>
where
    S: IpldStore + Clone + Send + Sync + 'static,
{
    let layer_cids = db::get_image_layer_cids(pool, reference).await?;

    // Verify all layers have been processed (have a head_cid)
    let missing_layers: Vec<_> = layer_cids
        .iter()
        .filter(|(_, head_cid)| head_cid.is_none())
        .map(|(digest, _)| digest.clone())
        .collect();

    if !missing_layers.is_empty() {
        return Err(MonocoreError::LayerHandling {
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Some layers have not been processed: {}",
                    missing_layers.join(", ")
                ),
            ),
            layer: missing_layers[0].clone(),
        });
    }

    // Load each layer's directory from its CID
    let mut layer_dirs = Vec::new();
    for (_, head_cid) in layer_cids {
        let cid = head_cid.unwrap().parse::<Cid>()?;
        let dir = Dir::load(&cid, store.clone()).await?;
        layer_dirs.push(dir);
    }

    Ok(layer_dirs)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
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
        pull_docker_registry_image(&image_ref, &download_dir).await?;

        // Initialize database connection for verification
        let db_path = monocore_home.join(OCI_DB_FILENAME);
        let pool = db::get_or_create_db_pool(&db_path, &OCI_DB_MIGRATOR).await?;

        // Verify database updates
        let layer_cids = db::get_image_layer_cids(&pool, &image_ref.to_string()).await?;
        assert!(!layer_cids.is_empty(), "Expected layers in database");

        // Verify all layers have head_cids
        for (digest, head_cid) in layer_cids {
            assert!(head_cid.is_some(), "Layer {} should have head_cid", digest);

            // Verify layer files exist
            let layer_path = download_dir.join(&digest);
            assert!(layer_path.exists(), "Layer file {} should exist", digest);

            // Verify extracted directories exist
            let extracted_path =
                download_dir.join(format!("{}.{}", digest, EXTRACTED_LAYER_SUFFIX));
            assert!(
                extracted_path.exists(),
                "Extracted directory {} should exist",
                digest
            );
            assert!(
                extracted_path.is_dir(),
                "Extracted path should be a directory"
            );
        }

        // Verify image has head_cid
        let image_complete = db::image_complete(&pool, &image_ref.to_string()).await?;
        assert!(image_complete, "Image should be marked as complete");

        // Verify final merged monofs image contains nginx files
        let store_path = monocore_home.join(BLOCKS_SUBDIR);
        let store = FlatFsStore::new(store_path);

        // Get the head CID from the database
        let image_record = sqlx::query("SELECT head_cid FROM images WHERE reference = ?")
            .bind(image_ref.to_string())
            .fetch_one(&pool)
            .await?;
        let head_cid_str = image_record
            .try_get::<Option<String>, _>("head_cid")?
            .expect("head_cid should not be null");

        helper::verify_nginx_files(&head_cid_str, store).await?;

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use super::*;

    /// Helper function to verify that all expected nginx files exist in the merged monofs image
    pub(super) async fn verify_nginx_files<S>(head_cid_str: &str, store: S) -> MonocoreResult<()>
    where
        S: IpldStore + Clone + Send + Sync + 'static,
    {
        let cid = head_cid_str.parse::<Cid>()?;
        let final_dir = Dir::load(&cid, store.clone()).await?;

        // Verify critical nginx paths and files exist
        let etc_dir = final_dir
            .get_dir("etc")
            .await?
            .expect("Merged image root should contain /etc directory");
        let nginx_dir = etc_dir
            .get_dir("nginx")
            .await?
            .expect("Merged image /etc should contain nginx directory");

        // Check nginx.conf exists
        let _nginx_conf = nginx_dir
            .get_file("nginx.conf")
            .await?
            .expect("nginx.conf should exist in /etc/nginx");

        // Check conf.d directory exists and contains default.conf
        let conf_d = nginx_dir
            .get_dir("conf.d")
            .await?
            .expect("conf.d directory should exist in /etc/nginx");
        let _default_conf = conf_d
            .get_file("default.conf")
            .await?
            .expect("default.conf should exist in /etc/nginx/conf.d");

        // Verify nginx binary exists in /usr/sbin
        let usr_sbin = final_dir
            .get_dir("usr")
            .await?
            .expect("Merged image root should contain /usr directory")
            .get_dir("sbin")
            .await?
            .expect("Merged image /usr should contain sbin directory");
        let _nginx_binary = usr_sbin
            .get_file("nginx")
            .await?
            .expect("nginx binary should exist in /usr/sbin");

        Ok(())
    }
}

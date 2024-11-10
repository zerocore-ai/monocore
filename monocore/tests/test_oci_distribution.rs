use monocore::{
    oci::distribution::{AuthProvider, DockerRegistry, OciRegistryPull},
    utils::{
        OCI_CONFIG_FILENAME, OCI_INDEX_FILENAME, OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME,
        OCI_REPO_SUBDIR, OCI_SUBDIR,
    },
};
use std::path::PathBuf;
use tempfile::tempdir;
use tokio::fs;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const DOCKER_AUTH_SERVICE: &str = "registry.docker.io";

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[test_log::test(tokio::test)]
#[ignore = "requires Docker registry authentication"]
async fn test_oci_distribution_docker_registry_authentication() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    let auth_material = registry
        .get_auth_material("library/alpine", DOCKER_AUTH_SERVICE, &["pull"])
        .await?;

    assert!(
        !auth_material.get_token().is_empty(),
        "Authentication token should not be empty"
    );
    assert!(
        !auth_material.get_access_token().is_empty(),
        "Access token should not be empty"
    );
    assert!(
        auth_material.get_expires_in() > &0,
        "Token expiration should be greater than 0"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires Docker registry access"]
async fn test_oci_distribution_docker_fetch_image_index() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    let index = registry
        .fetch_index("library/alpine", Some("latest"))
        .await?;

    assert!(
        !index.manifests().is_empty(),
        "Index should contain manifests"
    );
    assert!(index.schema_version() > 0, "Schema version should be valid");

    // Verify manifest platform information
    let manifest = index.manifests().first().unwrap();
    assert!(manifest.digest().to_string().starts_with("sha256:"));
    assert!(manifest.size() > 0);

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires Docker registry access and image download"]
async fn test_oci_distribution_docker_fetch_manifest_and_config() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // First fetch the index
    let index = registry
        .fetch_index("library/alpine", Some("latest"))
        .await?;
    let manifest_digest = &index.manifests()[0].digest();

    // Fetch manifest
    let manifest = registry
        .fetch_manifest("library/alpine", manifest_digest)
        .await?;

    assert!(
        !manifest.layers().is_empty(),
        "Manifest should contain layers"
    );
    assert!(manifest.config().size() > 0, "Config size should be valid");

    // Fetch config
    let config = registry
        .fetch_config("library/alpine", manifest.config().digest())
        .await?;

    assert!(config.config().is_some(), "Image config should be present");
    if let Some(img_config) = config.config() {
        let has_env = img_config
            .env()
            .as_ref()
            .map(|e| !e.is_empty())
            .unwrap_or(false);
        let has_cmd = img_config
            .cmd()
            .as_ref()
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        assert!(
            has_env || has_cmd,
            "Config should contain either env vars or cmd"
        );
    }

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Alpine Linux image"]
async fn test_oci_distribution_docker_pull_alpine_image() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull Alpine Linux image
    registry
        .pull_image("library/alpine", Some("latest"))
        .await?;

    // Verify the downloaded files and structure
    verify_oci_structure(temp_dir.path().to_path_buf(), "library_alpine__latest").await?;

    // Verify layer contents
    let manifest_path = temp_dir
        .path()
        .join(OCI_SUBDIR)
        .join(OCI_REPO_SUBDIR)
        .join("library_alpine__latest")
        .join(OCI_MANIFEST_FILENAME);

    let manifest_contents = fs::read_to_string(manifest_path).await?;
    let manifest: oci_spec::image::ImageManifest = serde_json::from_str(&manifest_contents)?;

    // Check each layer exists
    let layers_dir = temp_dir.path().join(OCI_SUBDIR).join(OCI_LAYER_SUBDIR);
    for layer in manifest.layers() {
        let layer_path = layers_dir.join(layer.digest().to_string());
        assert!(layer_path.exists(), "Layer {} not found", layer.digest());

        // Verify layer size
        let metadata = fs::metadata(&layer_path).await?;
        assert_eq!(
            metadata.len(),
            layer.size(),
            "Layer size mismatch for {}",
            layer.digest()
        );
    }

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Busybox image"]
async fn test_oci_distribution_docker_pull_busybox_image() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull Busybox image
    registry
        .pull_image("library/busybox", Some("latest"))
        .await?;

    // Verify the downloaded files and structure
    verify_oci_structure(temp_dir.path().to_path_buf(), "library_busybox__latest").await?;

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "tests error handling with nonexistent image"]
async fn test_oci_distribution_docker_pull_nonexistent_image() {
    let temp_dir = tempdir().unwrap();
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Try to pull a nonexistent image
    let result = registry
        .pull_image("library/nonexistentimage123456789", None)
        .await;

    assert!(result.is_err(), "Pulling nonexistent image should fail");
}

#[test_log::test(tokio::test)]
#[ignore = "requires pulling Node.js image (large, multiple layers)"]
async fn test_oci_distribution_docker_pull_image_with_multiple_layers() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let registry = DockerRegistry::with_path(temp_dir.path().to_path_buf());

    // Pull Node.js image (known to have multiple layers)
    registry.pull_image("library/node", Some("alpine")).await?;

    // Verify the downloaded files and structure
    verify_oci_structure(temp_dir.path().to_path_buf(), "library_node__alpine").await?;

    // Verify multiple layers exist
    let manifest_path = temp_dir
        .path()
        .join(OCI_SUBDIR)
        .join(OCI_REPO_SUBDIR)
        .join("library_node__alpine")
        .join(OCI_MANIFEST_FILENAME);

    let manifest_contents = fs::read_to_string(manifest_path).await?;
    let manifest: oci_spec::image::ImageManifest = serde_json::from_str(&manifest_contents)?;

    assert!(
        manifest.layers().len() > 1,
        "Node.js image should have multiple layers"
    );

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helper
//--------------------------------------------------------------------------------------------------

/// Helper function to verify the OCI directory structure and files after pulling an image
async fn verify_oci_structure(base_path: PathBuf, repo_tag: &str) -> anyhow::Result<()> {
    // Verify OCI directory exists
    let oci_dir = base_path.join(OCI_SUBDIR);
    assert!(oci_dir.exists(), "OCI directory does not exist");

    // Verify repo directory and files exist
    let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join(repo_tag);
    assert!(repo_dir.exists(), "Repository directory does not exist");

    // Check required files exist
    let required_files = [
        OCI_INDEX_FILENAME,
        OCI_MANIFEST_FILENAME,
        OCI_CONFIG_FILENAME,
    ];

    for file in required_files {
        let file_path = repo_dir.join(file);
        assert!(file_path.exists(), "Required file {} does not exist", file);
    }

    // Verify layers directory exists and contains files
    let layers_dir = oci_dir.join(OCI_LAYER_SUBDIR);
    assert!(layers_dir.exists(), "Layers directory does not exist");
    assert!(
        layers_dir.read_dir()?.next().is_some(),
        "Layers directory is empty"
    );

    Ok(())
}

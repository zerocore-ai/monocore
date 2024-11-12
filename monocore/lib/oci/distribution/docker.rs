use std::{
    ops::RangeBounds,
    path::{Path, PathBuf},
};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{future, stream::BoxStream, StreamExt};
use getset::{Getters, Setters};
use oci_spec::image::{Digest, ImageConfiguration, ImageIndex, ImageManifest, Os, Platform};
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

use crate::{
    utils::{
        self, OCI_CONFIG_FILENAME, OCI_INDEX_FILENAME, OCI_LAYER_SUBDIR, OCI_MANIFEST_FILENAME,
        OCI_REPO_SUBDIR, OCI_SUBDIR,
    },
    MonocoreError, MonocoreResult,
};

use super::{AuthProvider, OciRegistryPull};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Base URL for Docker Registry v2 API, used for accessing image manifests, layers, and other registry operations.
const DOCKER_REGISTRY_URL: &str = "https://registry-1.docker.io";

/// The service name used during token authentication, as specified by Docker's token-based authentication scheme.
const DOCKER_AUTH_SERVICE: &str = "registry.docker.io";

/// Endpoint for acquiring authentication tokens, as described in the Docker Registry authentication workflow.
const DOCKER_AUTH_REALM: &str = "https://auth.docker.io/token";

/// The MIME type for Docker Registry v2 manifests, used to identify the format of the manifest data.
const DOCKER_MANIFEST_MIME_TYPE: &str = "application/vnd.docker.distribution.manifest.v2+json";

/// The MIME type for Docker Registry v2 manifest lists, used to identify the format of the manifest list data.
const DOCKER_MANIFEST_LIST_MIME_TYPE: &str =
    "application/vnd.docker.distribution.manifest.list.v2+json";

/// The MIME type for Docker Registry v2 image blobs, used to identify the format of the image blob data.
const DOCKER_IMAGE_BLOB_MIME_TYPE: &str = "application/vnd.docker.image.rootfs.diff.tar.gzip";

/// The MIME type for Docker Registry v2 configuration blobs, used to identify the format of the configuration blob data.
const DOCKER_CONFIG_MIME_TYPE: &str = "application/vnd.docker.container.image.v1+json";

/// The annotation key used to identify attestation manifests in the Docker Registry.
const DOCKER_REFERENCE_TYPE_ANNOTATION: &str = "vnd.docker.reference.type";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// DockerRegistry is a client for interacting with Docker's Registry HTTP API v2.
/// It handles authentication, image manifest retrieval, and blob fetching.
///
/// [See OCI distribution specification for more details on the manifest schema][OCI Distribution Spec]
///
/// [See Docker Registry API for more details on the API][Docker Registry API]
///
/// [OCI Distribution Spec]: https://distribution.github.io/distribution/spec/manifest-v2-2/#image-manifest-version-2-schema-2
/// [Docker Registry API]: https://distribution.github.io/distribution/spec/api/#introduction
#[derive(Debug, Getters, Setters)]
#[getset(get = "pub with_prefix", set = "pub with_prefix")]
pub struct DockerRegistry {
    /// The HTTP client used to make requests to the Docker registry.
    client: ClientWithMiddleware,

    /// The path to the OCI directory where artifacts like repositories metadata and layers are stored.
    oci_dir: PathBuf,
}

/// Stores authentication credentials obtained from the Docker registry, including tokens and expiration details.
#[derive(Debug, Serialize, Deserialize, Getters, Setters)]
#[getset(get = "pub with_prefix", set = "pub with_prefix")]
pub struct DockerAuthMaterial {
    /// The token used to authenticate requests to the Docker registry.
    token: String,

    /// The access token used to authenticate requests to the Docker registry.
    access_token: String,

    /// The expiration time of the access token.
    expires_in: u32,

    /// The time the access token was issued.
    issued_at: DateTime<Utc>,
}

/// Represents a response from the Docker registry, which could either be successful (`Ok`) or an error (`Error`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DockerRegistryResponse<T> {
    /// Represents a successful response from the Docker registry.
    Ok(T),

    /// Represents an error response from the Docker registry.
    Error(DockerRegistryResponseError),
}

/// Represents an error response from the Docker registry, including detailed error messages.
#[derive(Debug, Serialize, Deserialize, Error)]
#[error("docker registry error: {errors}")]
pub struct DockerRegistryResponseError {
    /// The errors returned by the Docker registry.
    errors: serde_json::Value,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

/// Creates a new instance of `DockerRegistry` with an HTTP client configured for retrying transient errors.
/// This client is used to interact with the Docker Registry HTTP API.
impl DockerRegistry {
    /// Creates a new DockerRegistry instance with the default artifacts directory (MONOCORE_HOME).
    pub fn new() -> Self {
        Self::with_oci_dir(utils::monocore_home_path().join(OCI_SUBDIR))
    }

    /// Creates a new DockerRegistry instance with a custom base path.
    ///
    /// This is useful for testing or when you need to store OCI artifacts
    /// in a different location than the default MONOCORE_HOME.
    ///
    /// ## Arguments
    /// * `oci_dir` - The base path where OCI artifacts will be stored. The OCI directory
    ///               structure will be created under this path.
    pub fn with_oci_dir(oci_dir: PathBuf) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client_builder = ClientBuilder::new(Client::new());
        let client = client_builder
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self { client, oci_dir }
    }

    /// Gets the size of a downloaded file if it exists.
    fn get_downloaded_file_size(&self, path: &Path) -> u64 {
        // If the file does not exist, return 0 indicating no bytes have been downloaded
        if !path.exists() {
            return 0;
        }

        path.metadata().unwrap().len()
    }

    /// Downloads a blob from the registry, supports download resumption if the file already partially exists.
    async fn download_image_blob(
        &self,
        repository: &str,
        digest: &Digest,
        download_size: u64,
        destination: PathBuf,
    ) -> MonocoreResult<()> {
        // Ensure the destination directory exists
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Get the size of the already downloaded file if it exists
        let downloaded_size = self.get_downloaded_file_size(&destination);

        // Open the file for writing, create if it doesn't exist
        let mut file = if downloaded_size == 0 {
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&destination)
                .await?
        } else if downloaded_size < download_size {
            OpenOptions::new().append(true).open(&destination).await?
        } else {
            tracing::info!(
                "file already exists skipping download: {}",
                destination.display()
            );
            return Ok(());
        };

        let mut stream = self
            .fetch_image_blob(repository, digest, downloaded_size..)
            .await?;

        // Write the stream to the file
        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            file.write_all(&bytes).await?;
        }

        // Verify the hash of the downloaded file
        let algorithm = digest.algorithm();
        let expected_hash = digest.digest();
        let actual_hash = hex::encode(utils::get_file_hash(&destination, algorithm).await?);

        // Delete the already downloaded file if the hash does not match
        if actual_hash != expected_hash {
            fs::remove_file(destination).await?;
            return Err(MonocoreError::ImageLayerDownloadFailed(format!(
                "({repository}:{digest}) file hash {actual_hash} does not match expected hash {expected_hash}",
            )));
        }

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait::async_trait]
impl AuthProvider for DockerRegistry {
    type AuthMaterial = DockerAuthMaterial;

    /// Gets the necessary authentication credentials for the given repository and tag.
    ///
    /// Currently, Docker tokens expire after 300 seconds, so we need to re-authenticate
    /// after that period or just fetch new tokens on each request.
    async fn get_auth_material(
        &self,
        repository: &str,
        service: &str,
        scopes: &[&str],
    ) -> MonocoreResult<Self::AuthMaterial> {
        let request = self
            .client
            .get(DOCKER_AUTH_REALM)
            .query(&[
                ("service", service),
                (
                    "scope",
                    format!("repository:{}:{}", repository, scopes.join(",")).as_str(),
                ),
            ])
            .build()?;

        let response = self.client.execute(request).await?;
        let auth_credentials = response.json::<DockerAuthMaterial>().await?;

        Ok(auth_credentials)
    }
}

#[async_trait::async_trait]
impl OciRegistryPull for DockerRegistry {
    async fn pull_image(&self, repository: &str, tag: Option<&str>) -> MonocoreResult<()> {
        let tag = tag.unwrap_or("latest");
        let repo_tag = format!(
            "{}__{}",
            utils::sanitize_repo_name(repository),
            utils::sanitize_repo_name(tag)
        );

        // Create the repository tag directory
        let repo_tag_dir = self.oci_dir.join(OCI_REPO_SUBDIR).join(&repo_tag);

        fs::create_dir_all(&repo_tag_dir).await?;

        // Fetch and save index
        let index = self.fetch_index(repository, Some(tag)).await?;
        let index_path = repo_tag_dir.join(OCI_INDEX_FILENAME);
        fs::write(&index_path, serde_json::to_string_pretty(&index)?).await?;

        // Select the right manifest for the platform or choose first if not specified
        let platform = Platform::default();
        let manifest_desc = index
            .manifests()
            .iter()
            .find(|m| {
                m.platform().as_ref().map_or(false, |p| {
                    // First priority: match both Linux OS and architecture
                    matches!(p.os(), Os::Linux) &&
                    p.architecture() == platform.architecture() &&
                    // Skip attestation manifests
                    !m.annotations().as_ref().map_or(false, |a| a.contains_key(DOCKER_REFERENCE_TYPE_ANNOTATION))
                })
            })
            .or_else(|| {
                // Second priority: match architecture only, if no Linux match found
                index.manifests().iter().find(|m| {
                    m.platform().as_ref().map_or(false, |p| {
                        p.architecture() == platform.architecture() &&
                        !m.annotations().as_ref().map_or(false, |a| a.contains_key(DOCKER_REFERENCE_TYPE_ANNOTATION))
                    })
                })
            })
            .ok_or(MonocoreError::ManifestNotFound)?;

        // Fetch and save manifest
        let manifest = self
            .fetch_manifest(repository, manifest_desc.digest())
            .await?;
        let manifest_path = repo_tag_dir.join(OCI_MANIFEST_FILENAME);
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?).await?;

        // Fetch and save config
        let config = self
            .fetch_config(repository, manifest.config().digest())
            .await?;
        let config_path = repo_tag_dir.join(OCI_CONFIG_FILENAME);
        fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;

        // Download layers concurrently
        let layer_futures: Vec<_> = manifest
            .layers()
            .iter()
            .map(|layer_desc| {
                let layer_path = self
                    .oci_dir
                    .join(OCI_LAYER_SUBDIR)
                    .join(layer_desc.digest().to_string());

                self.download_image_blob(
                    repository,
                    layer_desc.digest(),
                    layer_desc.size(),
                    layer_path,
                )
            })
            .collect();

        // Wait for all layers to download
        for result in future::join_all(layer_futures).await {
            result?;
        }

        Ok(())
    }

    async fn fetch_index(&self, repository: &str, tag: Option<&str>) -> MonocoreResult<ImageIndex> {
        let token = self
            .get_auth_material(repository, DOCKER_AUTH_SERVICE, &["pull"])
            .await?
            .token;

        let tag = tag.unwrap_or("latest");

        let request = self
            .client
            .get(format!(
                "{}/v2/{}/manifests/{}",
                DOCKER_REGISTRY_URL, repository, tag
            ))
            .bearer_auth(token)
            .header("Accept", DOCKER_MANIFEST_LIST_MIME_TYPE)
            .build()?;

        let response = self.client.execute(request).await?;
        let image_index = response
            .json::<DockerRegistryResponse<ImageIndex>>()
            .await?;

        match image_index {
            DockerRegistryResponse::Ok(index) => Ok(index),
            DockerRegistryResponse::Error(err) => Err(err.into()),
        }
    }

    async fn fetch_manifest(
        &self,
        repository: &str,
        digest: &Digest,
    ) -> MonocoreResult<ImageManifest> {
        let token = self
            .get_auth_material(repository, DOCKER_AUTH_SERVICE, &["pull"])
            .await?
            .token;

        let request = self
            .client
            .get(format!(
                "{}/v2/{}/manifests/{}",
                DOCKER_REGISTRY_URL, repository, digest
            ))
            .bearer_auth(token)
            .header("Accept", DOCKER_MANIFEST_MIME_TYPE)
            .build()?;

        let response = self.client.execute(request).await?;
        let manifest = response
            .json::<DockerRegistryResponse<ImageManifest>>()
            .await?;

        match manifest {
            DockerRegistryResponse::Ok(manifest) => Ok(manifest),
            DockerRegistryResponse::Error(err) => Err(err.into()),
        }
    }

    async fn fetch_config(
        &self,
        repository: &str,
        digest: &Digest,
    ) -> MonocoreResult<ImageConfiguration> {
        let token = self
            .get_auth_material(repository, DOCKER_AUTH_SERVICE, &["pull"])
            .await?
            .token;

        let request = self
            .client
            .get(format!(
                "{}/v2/{}/blobs/{}",
                DOCKER_REGISTRY_URL, repository, digest
            ))
            .bearer_auth(token)
            .header("Accept", DOCKER_CONFIG_MIME_TYPE)
            .build()?;

        let response = self.client.execute(request).await?;
        let config = response
            .json::<DockerRegistryResponse<ImageConfiguration>>()
            .await?;

        match config {
            DockerRegistryResponse::Ok(config) => Ok(config),
            DockerRegistryResponse::Error(err) => Err(err.into()),
        }
    }

    async fn fetch_image_blob(
        &self,
        repository: &str,
        digest: &Digest,
        range: impl RangeBounds<u64> + Send,
    ) -> MonocoreResult<BoxStream<'static, MonocoreResult<Bytes>>> {
        let (start, end) = utils::convert_bounds(range);
        let end = if end == u64::MAX {
            "".to_string()
        } else {
            end.to_string()
        };

        tracing::info!("fetching blob: {repository} {digest} {start}-{end}");

        let token = self
            .get_auth_material(repository, DOCKER_AUTH_SERVICE, &["pull"])
            .await?
            .token;

        let request = self
            .client
            .get(format!(
                "{}/v2/{}/blobs/{}",
                DOCKER_REGISTRY_URL, repository, digest
            ))
            .bearer_auth(token)
            .header("Accept", DOCKER_IMAGE_BLOB_MIME_TYPE)
            .header("Range", format!("bytes={start}-{end}"))
            .build()?;

        let response = self.client.execute(request).await?;
        let stream = response
            .bytes_stream()
            .map(|item| item.map_err(|e| e.into()));

        Ok(stream.boxed())
    }
}

impl Default for DockerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

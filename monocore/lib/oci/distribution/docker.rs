use std::{
    ops::RangeBounds,
    path::{Path, PathBuf},
};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{future, stream::BoxStream, StreamExt};
use getset::{Getters, Setters};
use oci_spec::image::{Digest, ImageConfiguration, ImageIndex, ImageManifest, Platform};
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
    utils::{self, IMAGE_LAYERS_SUBDIR},
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
#[getset(get = "pub", set = "pub")]
pub struct DockerRegistry {
    /// The HTTP client used to make requests to the Docker registry.
    client: ClientWithMiddleware,

    /// The path to the where files are downloaded.
    path: PathBuf,
}

/// Stores authentication credentials obtained from the Docker registry, including tokens and expiration details.
#[derive(Debug, Serialize, Deserialize, Getters, Setters)]
#[getset(get = "pub", set = "pub")]
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
    /// Creates a new DockerRegistry instance with an HTTP client configured for retrying transient errors.
    pub fn new() -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client_builder = ClientBuilder::new(Client::new());
        let client = client_builder
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            client,
            path: utils::monocore_home_path(),
        }
    }

    /// Creates a new DockerRegistry instance with a custom path.
    pub fn with_path(path: PathBuf) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client_builder = ClientBuilder::new(Client::new());
        let client = client_builder
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self { client, path }
    }

    /// Gets the size of a downloaded file if it exists.
    fn get_downloaded_file_size(&self, path: &Path) -> u64 {
        // If the file does not exist, return 0
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

        // TODO: Check that the downloaded file has the same digest as the one we wanted
        // TODO: Use the hash method derived from the digest to verify the download
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
        let index = self.fetch_index(repository, tag).await?;

        // Select the right manifest for the platform or choose first if not specified
        let platform = Platform::default();
        let manifest = index
            .manifests()
            .iter()
            .find(|m| m.platform().as_ref().map_or(false, |p| p == &platform))
            .or_else(|| index.manifests().first())
            .ok_or(MonocoreError::ManifestNotFound)?;

        // Fetch the manifest
        let manifest = self.fetch_manifest(repository, manifest.digest()).await?;

        // Fetch the config
        let config_future = self.fetch_config(repository, manifest.config().digest());

        // Download layers concurrently
        let layer_futures: Vec<_> = manifest
            .layers()
            .iter()
            .map(|layer| {
                let layer_path = self
                    .path
                    .join(IMAGE_LAYERS_SUBDIR)
                    .join(layer.digest().to_string());
                self.download_image_blob(repository, layer.digest(), layer.size(), layer_path)
            })
            .collect();

        let results = future::join_all(layer_futures);

        let (config, results) = tokio::join!(config_future, results);

        // Check for errors in the config fetch
        config?;

        // Check for errors in the download results
        for result in results {
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test_log::test(tokio::test)]
    async fn test_authenticate() -> anyhow::Result<()> {
        let registry = DockerRegistry::new();

        let auth_material = registry
            .get_auth_material("library/alpine", DOCKER_AUTH_SERVICE, &["pull"])
            .await;

        assert!(auth_material.is_ok());

        Ok(())
    }

    #[ignore]
    #[test_log::test(tokio::test)]
    async fn test_fetch_index() -> anyhow::Result<()> {
        let registry = DockerRegistry::new();

        let index = registry
            .fetch_index("library/alpine", Some("latest"))
            .await?;

        tracing::info!("index: {:?}", index);

        assert!(index.manifests().len() > 0);

        Ok(())
    }

    #[ignore]
    #[test_log::test(tokio::test)]
    async fn test_fetch_manifest() -> anyhow::Result<()> {
        let registry = DockerRegistry::new();

        let index = registry
            .fetch_index("library/alpine", Some("latest"))
            .await?;

        tracing::info!("index: {:?}", index);

        let manifest = registry
            .fetch_manifest("library/alpine", &index.manifests()[0].digest())
            .await?;

        tracing::info!("manifest: {:?}", manifest);

        assert!(manifest.layers().len() > 0);

        Ok(())
    }

    #[test_log::test(tokio::test)]
    async fn test_fetches() -> anyhow::Result<()> {
        let registry = DockerRegistry::new();

        let index = registry
            .fetch_index("library/alpine", Some("latest"))
            .await?;

        tracing::info!("index: {:?}", index);

        let manifest = registry
            .fetch_manifest("library/alpine", &index.manifests()[0].digest())
            .await?;

        tracing::info!("manifest: {:?}", manifest);

        let config = registry
            .fetch_config("library/alpine", &manifest.config().digest())
            .await?;

        tracing::info!("config: {:?}", config);

        assert!(config.config().is_some());

        Ok(())
    }

    #[test_log::test(tokio::test)]
    async fn test_pull_image() -> anyhow::Result<()> {
        let registry = DockerRegistry::new();

        let result = registry.pull_image("library/alpine", None).await;

        assert!(result.is_ok());

        Ok(())
    }
}

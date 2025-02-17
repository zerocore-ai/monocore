use std::{
    ops::RangeBounds,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{future, stream::BoxStream, StreamExt};
use getset::{Getters, Setters};
use oci_spec::image::{Digest, ImageConfiguration, ImageIndex, ImageManifest, Os, Platform};
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use thiserror::Error;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

use crate::{
    management::{self, OCI_DB_MIGRATOR},
    oci::{OciRegistryPull, ReferenceSelector},
    utils, MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The domain name of the Docker registry, used to construct image references.
pub const DOCKER_REFERENCE_REGISTRY_DOMAIN: &str = "docker.io";

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

    /// The directory where image layers are downloaded.
    layer_download_dir: PathBuf,

    /// The database where image configurations, indexes, and manifests are stored.
    oci_db: Pool<Sqlite>,
}

//--------------------------------------------------------------------------------------------------
// Types: Models
//--------------------------------------------------------------------------------------------------

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

impl DockerRegistry {
    /// Creates a new Docker Registry client with the specified image download path and OCI database path.
    ///
    /// ## Arguments
    ///
    /// * `layer_download_dir` - The directory where downloaded image layers will be stored
    /// * `oci_db_path` - The path to the SQLite database that stores OCI-related metadata
    pub async fn new(
        layer_download_dir: impl Into<PathBuf>,
        oci_db_path: impl AsRef<Path>,
    ) -> MonocoreResult<Self> {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client_builder = ClientBuilder::new(Client::new());
        let client = client_builder
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            client,
            layer_download_dir: layer_download_dir.into(),
            oci_db: management::get_or_create_db_pool(oci_db_path.as_ref(), &OCI_DB_MIGRATOR)
                .await?,
        })
    }

    /// Gets the size of a downloaded file if it exists.
    fn get_downloaded_file_size(&self, digest: &Digest) -> u64 {
        let download_path = self.layer_download_dir.join(digest.to_string());
        // If the file does not exist, return 0 indicating no bytes have been downloaded
        if !download_path.exists() {
            return 0;
        }

        download_path.metadata().unwrap().len()
    }

    /// Gets the necessary authentication credentials for the given repository and tag.
    ///
    /// Currently, Docker tokens expire after 300 seconds, so we need to re-authenticate
    /// after that period or just fetch new tokens on each request.
    async fn get_access_credentials(
        &self,
        repository: &str,
        service: &str,
        scopes: &[&str],
    ) -> MonocoreResult<DockerAuthMaterial> {
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

    /// Downloads a blob from the registry, supports download resumption if the file already partially exists.
    pub async fn download_image_blob(
        &self,
        repository: &str,
        digest: &Digest,
        download_size: u64,
    ) -> MonocoreResult<()> {
        let download_path = self.layer_download_dir.join(digest.to_string());

        // Ensure the destination directory exists
        if let Some(parent) = download_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Get the size of the already downloaded file if it exists
        let downloaded_size = self.get_downloaded_file_size(digest);

        // Open the file for writing, create if it doesn't exist
        let mut file = if downloaded_size == 0 {
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&download_path)
                .await?
        } else if downloaded_size < download_size {
            OpenOptions::new().append(true).open(&download_path).await?
        } else {
            tracing::info!(
                "file already exists skipping download: {}",
                download_path.display()
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
        let actual_hash = hex::encode(utils::get_file_hash(&download_path, algorithm).await?);

        // Delete the already downloaded file if the hash does not match
        if actual_hash != expected_hash {
            fs::remove_file(&download_path).await?;
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

#[async_trait]
impl OciRegistryPull for DockerRegistry {
    async fn pull_image(
        &self,
        repository: &str,
        selector: ReferenceSelector,
    ) -> MonocoreResult<()> {
        // Calculate total size and save image record
        let index = self.fetch_index(repository, selector.clone()).await?;
        let total_size: i64 = index.manifests().iter().map(|m| m.size() as i64).sum();

        // Construct reference based on selector type
        let reference = match &selector {
            ReferenceSelector::Tag { tag, digest } => {
                let digest_part = digest
                    .as_ref()
                    .map(|d| format!("@{}:{}", d.algorithm(), d.digest()))
                    .unwrap_or_default();
                format!("{DOCKER_REFERENCE_REGISTRY_DOMAIN}/{repository}:{tag}{digest_part}")
            }
            ReferenceSelector::Digest(digest) => {
                let digest_part = format!("@{}:{}", digest.algorithm(), digest.digest());
                format!("{DOCKER_REFERENCE_REGISTRY_DOMAIN}/{repository}{digest_part}")
            }
        };

        let image_id =
            management::save_or_update_image(&self.oci_db, &reference, total_size).await?;

        // Save index
        let platform = Platform::default();
        let index_id =
            management::save_index(&self.oci_db, image_id, &index, Some(&platform)).await?;

        // Select the right manifest for the platform or choose first if not specified
        let manifest_desc = index
            .manifests()
            .iter()
            .find(|m| {
                m.platform().as_ref().is_some_and(|p| {
                    // First priority: match both Linux OS and architecture
                    matches!(p.os(), Os::Linux) &&
                    p.architecture() == platform.architecture() &&
                    // Skip attestation manifests
                    !m.annotations().as_ref().is_some_and(|a| a.contains_key(DOCKER_REFERENCE_TYPE_ANNOTATION))
                })
            })
            .or_else(|| {
                // Second priority: match architecture only, if no Linux match found
                index.manifests().iter().find(|m| {
                    m.platform().as_ref().is_some_and(|p| {
                        p.architecture() == platform.architecture() &&
                        !m.annotations().as_ref().is_some_and(|a| a.contains_key(DOCKER_REFERENCE_TYPE_ANNOTATION))
                    })
                })
            })
            .ok_or(MonocoreError::ManifestNotFound)?;

        // Fetch and save manifest
        let manifest = self
            .fetch_manifest(repository, manifest_desc.digest())
            .await?;
        let manifest_id =
            management::save_manifest(&self.oci_db, image_id, Some(index_id), &manifest).await?;

        // Fetch and save config
        let config = self
            .fetch_config(repository, manifest.config().digest())
            .await?;
        management::save_config(&self.oci_db, manifest_id, &config).await?;

        // Download layers concurrently and save to database
        let layer_futures: Vec<_> = manifest
            .layers()
            .iter()
            .zip(config.rootfs().diff_ids())
            .map(|(layer_desc, diff_id)| async {
                // Check if layer already exists in database
                if management::layer_exists(&self.oci_db, &layer_desc.digest().to_string()).await? {
                    tracing::info!(
                        "layer {} already exists, skipping download",
                        layer_desc.digest()
                    );
                } else {
                    // Download the layer if it doesn't exist
                    self.download_image_blob(repository, layer_desc.digest(), layer_desc.size())
                        .await?;
                }

                // Save layer metadata to database
                management::save_or_update_layer(
                    &self.oci_db,
                    manifest_id,
                    &layer_desc.media_type().to_string(),
                    &layer_desc.digest().to_string(),
                    layer_desc.size() as i64,
                    diff_id,
                )
                .await?;

                Ok::<_, MonocoreError>(())
            })
            .collect();

        // Wait for all layers to download and save
        for result in future::join_all(layer_futures).await {
            result?;
        }

        Ok(())
    }

    async fn fetch_index(
        &self,
        repository: &str,
        selector: ReferenceSelector,
    ) -> MonocoreResult<ImageIndex> {
        let token = self
            .get_access_credentials(repository, DOCKER_AUTH_SERVICE, &["pull"])
            .await?
            .token;

        // Construct URL based on selector type
        let reference = match &selector {
            ReferenceSelector::Tag { tag, digest } => {
                let digest_part = digest
                    .as_ref()
                    .map(|d| format!("@{}:{}", d.algorithm(), d.digest()))
                    .unwrap_or_default();
                format!("{tag}{digest_part}")
            }
            ReferenceSelector::Digest(digest) => {
                format!("@{}:{}", digest.algorithm(), digest.digest())
            }
        };

        let request = self
            .client
            .get(format!(
                "{}/v2/{}/manifests/{}",
                DOCKER_REGISTRY_URL, repository, reference
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
            .get_access_credentials(repository, DOCKER_AUTH_SERVICE, &["pull"])
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
            .get_access_credentials(repository, DOCKER_AUTH_SERVICE, &["pull"])
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
            .get_access_credentials(repository, DOCKER_AUTH_SERVICE, &["pull"])
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use oci_spec::image::{DigestAlgorithm, Os};
    use sqlx::Row;
    use tokio::test;

    #[test]
    #[ignore = "makes network requests to Docker registry to pull an image"]
    async fn test_docker_pull_image() -> anyhow::Result<()> {
        let (client, temp_download_dir, _temp_db_dir) = helper::setup_test_client().await;
        let repository = "library/alpine";
        let tag = "latest";
        let result = client
            .pull_image(repository, ReferenceSelector::tag(tag))
            .await;
        assert!(result.is_ok());

        // Verify image record in database
        let image = sqlx::query("SELECT * FROM images WHERE reference = ?")
            .bind(format!(
                "{DOCKER_REFERENCE_REGISTRY_DOMAIN}/{repository}:{tag}"
            ))
            .fetch_one(&client.oci_db)
            .await?;
        assert!(image.get::<i64, _>("size_bytes") > 0);

        // Verify index record
        let index_id = image.get::<i64, _>("id");
        let index = sqlx::query("SELECT * FROM indexes WHERE image_id = ?")
            .bind(index_id)
            .fetch_one(&client.oci_db)
            .await?;
        assert_eq!(index.get::<i64, _>("schema_version"), 2);

        // Verify manifest record
        let manifest = sqlx::query("SELECT * FROM manifests WHERE image_id = ?")
            .bind(index_id)
            .fetch_one(&client.oci_db)
            .await?;
        assert_eq!(manifest.get::<i64, _>("schema_version"), 2);

        // Verify config record
        let manifest_id = manifest.get::<i64, _>("id");
        let config = sqlx::query("SELECT * FROM configs WHERE manifest_id = ?")
            .bind(manifest_id)
            .fetch_one(&client.oci_db)
            .await?;
        assert!(matches!(config.get::<String, _>("os"), s if s == Os::Linux.to_string()));

        // Verify layers were downloaded and match records
        let layers = sqlx::query("SELECT * FROM layers WHERE manifest_id = ?")
            .bind(manifest_id)
            .fetch_all(&client.oci_db)
            .await?;
        assert!(!layers.is_empty());

        for layer in layers {
            let digest = layer.get::<String, _>("digest");
            let size = layer.get::<i64, _>("size_bytes");
            let layer_path = temp_download_dir.path().join(&digest);

            // Verify layer file exists and has correct size
            assert!(layer_path.exists(), "Layer file {} not found", digest);
            assert_eq!(
                fs::metadata(&layer_path).await?.len() as i64,
                size,
                "Layer {} size mismatch",
                digest
            );

            // Verify layer hash
            let parts: Vec<&str> = digest.split(':').collect();
            let algorithm = &DigestAlgorithm::try_from(parts[0])?;
            let expected_hash = parts[1];
            let actual_hash = hex::encode(utils::get_file_hash(&layer_path, algorithm).await?);
            assert_eq!(actual_hash, expected_hash, "Layer {} hash mismatch", digest);
        }

        Ok(())
    }

    #[test]
    #[ignore = "makes network requests to Docker registry to fetch image index"]
    async fn test_docker_fetch_index() -> anyhow::Result<()> {
        let (client, _temp_download_dir, _temp_db_dir) = helper::setup_test_client().await;
        let repository = "library/alpine";
        let tag = "latest";

        let result = client
            .fetch_index(repository, ReferenceSelector::tag(tag))
            .await;
        assert!(result.is_ok());

        let index = result.unwrap();
        assert!(!index.manifests().is_empty());

        // Verify manifest entries have required fields
        for manifest in index.manifests() {
            assert!(manifest.size() > 0);
            assert!(manifest.digest().to_string().starts_with("sha256:"));
            assert!(manifest.media_type().to_string().contains("manifest"));

            // Verify platform info for non-attestation manifests
            if !manifest
                .annotations()
                .as_ref()
                .is_some_and(|a| a.contains_key(DOCKER_REFERENCE_TYPE_ANNOTATION))
            {
                let platform = manifest.platform().as_ref().expect("Platform info missing");
                assert!(matches!(platform.os(), Os::Linux));
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "makes network requests to Docker registry to fetch image manifest"]
    async fn test_docker_fetch_manifest() -> anyhow::Result<()> {
        let (client, _temp_download_dir, _temp_db_dir) = helper::setup_test_client().await;
        let repository = "library/alpine";

        // First get the manifest digest from the index
        let index = client
            .fetch_index(repository, ReferenceSelector::tag("latest"))
            .await?;

        let manifest_desc = index.manifests().first().unwrap();
        let result = client
            .fetch_manifest(repository, manifest_desc.digest())
            .await;

        assert!(result.is_ok());
        let manifest = result.unwrap();

        // Verify manifest has required fields
        assert_eq!(manifest.schema_version(), 2);
        assert!(manifest.config().size() > 0);
        assert!(manifest
            .config()
            .digest()
            .to_string()
            .starts_with("sha256:"));
        assert!(manifest
            .config()
            .media_type()
            .to_string()
            .contains("config"));

        // Verify layers
        assert!(!manifest.layers().is_empty());
        for layer in manifest.layers() {
            assert!(layer.size() > 0);
            assert!(layer.digest().to_string().starts_with("sha256:"));
            assert!(layer.media_type().to_string().contains("layer"));
        }

        Ok(())
    }

    #[test]
    #[ignore = "makes network requests to Docker registry to fetch image config"]
    async fn test_docker_fetch_config() -> anyhow::Result<()> {
        let (client, _temp_download_dir, _temp_db_dir) = helper::setup_test_client().await;
        let repository = "library/alpine";

        // Get the config digest from manifest
        let index = client
            .fetch_index(repository, ReferenceSelector::tag("latest"))
            .await?;

        let manifest = client
            .fetch_manifest(repository, index.manifests().first().unwrap().digest())
            .await?;

        let result = client
            .fetch_config(repository, manifest.config().digest())
            .await;
        assert!(result.is_ok());

        let config = result.unwrap();

        // Verify required OCI spec fields
        assert_eq!(*config.os(), Os::Linux);
        assert!(config.rootfs().typ() == "layers");
        assert!(!config.rootfs().diff_ids().is_empty());

        // Verify optional but common fields
        if let Some(created) = config.created() {
            let created_time = DateTime::parse_from_rfc3339(created).unwrap();
            assert!(created_time.timestamp_millis() > 0);
        }
        if let Some(config_fields) = config.config() {
            if let Some(env) = config_fields.env() {
                assert!(!env.is_empty());
            }
            if let Some(cmd) = config_fields.cmd() {
                assert!(!cmd.is_empty());
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "makes network requests to Docker registry to fetch image blob"]
    async fn test_docker_fetch_image_blob() -> anyhow::Result<()> {
        let (client, temp_download_dir, _temp_db_dir) = helper::setup_test_client().await;
        let repository = "library/alpine";

        // Get a layer digest from manifest
        let index = client
            .fetch_index(repository, ReferenceSelector::tag("latest"))
            .await?;

        let manifest = client
            .fetch_manifest(repository, index.manifests().first().unwrap().digest())
            .await?;

        let layer = manifest.layers().first().unwrap();
        let mut stream = client
            .fetch_image_blob(repository, layer.digest(), 0..)
            .await?;

        // Download the blob to a temporary file
        let temp_file = temp_download_dir.path().join("test_blob");
        let mut file = fs::File::create(&temp_file).await?;
        let mut total_size = 0;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            total_size += bytes.len();
            file.write_all(&bytes).await?;
        }

        // Verify size matches
        assert!(total_size > 0);
        assert_eq!(total_size as u64, layer.size());

        // Verify hash matches
        let algorithm = layer.digest().algorithm();
        let expected_hash = layer.digest().digest();
        let actual_hash = hex::encode(utils::get_file_hash(&temp_file, algorithm).await?);
        assert_eq!(actual_hash, expected_hash);

        Ok(())
    }

    #[test]
    #[ignore = "makes network requests to Docker registry to get authentication credentials"]
    async fn test_docker_get_access_credentials() -> anyhow::Result<()> {
        let (client, _temp_download_dir, _temp_db_dir) = helper::setup_test_client().await;

        let result = client
            .get_access_credentials("library/alpine", DOCKER_AUTH_SERVICE, &["pull"])
            .await;

        assert!(result.is_ok());
        let credentials = result.unwrap();

        // Verify credential fields
        assert!(!credentials.token.is_empty());
        assert!(!credentials.access_token.is_empty());
        assert!(credentials.expires_in > 0);

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use tempfile::TempDir;

    use super::*;

    // Helper function to create a test Docker registry client
    pub(super) async fn setup_test_client() -> (DockerRegistry, TempDir, TempDir) {
        let temp_download_dir = TempDir::new().unwrap();
        let temp_db_dir = TempDir::new().unwrap();
        let db_path = temp_db_dir.path().join("test.db");

        let client = DockerRegistry::new(temp_download_dir.path().to_path_buf(), db_path)
            .await
            .unwrap();

        (client, temp_download_dir, temp_db_dir)
    }
}

use std::ops::RangeBounds;

use crate::MonocoreResult;
use bytes::Bytes;
use futures::stream::BoxStream;
use oci_spec::image::{Digest, ImageConfiguration, ImageIndex, ImageManifest};

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// Trait for establishing authentication necessary for a registry client.
#[async_trait::async_trait]
pub trait AuthProvider {
    /// The type of the authentication material.
    type AuthMaterial;

    /// Runs the authentication process for the registry client and returns the authentication material.
    async fn get_auth_material(
        &self,
        repository: &str,
        service: &str,
        scopes: &[&str],
    ) -> MonocoreResult<Self::AuthMaterial>;
}

/// Trait defining methods for interacting with an OCI-compliant registry,
/// including pulling images, fetching manifests, and fetching blobs.
#[async_trait::async_trait]
pub trait OciRegistryPull {
    /// Pulls an OCI image from the specified repository. This includes downloading
    /// the image manifest, fetching the image configuration, and downloading the image layers.
    ///
    /// If no tag is provided, defaults to the "latest" tag according to OCI specifications.
    async fn pull_image(
        &self,
        repository: &str,
        tag: Option<&str>, // Defaults to "latest"
    ) -> MonocoreResult<()>;

    /// Fetches the image index (manifest list) for multi-platform support.
    /// Retrieves the appropriate manifest for the target platform.
    async fn fetch_index(
        &self,
        repository: &str,
        tag: Option<&str>, // Defaults to "latest"
    ) -> MonocoreResult<ImageIndex>;

    /// Fetches an image manifest by digest.
    /// Provides the list of layers and configurations for an image.
    async fn fetch_manifest(
        &self,
        repository: &str,
        digest: &Digest,
    ) -> MonocoreResult<ImageManifest>;

    /// Fetches the image configuration by digest.
    /// Returns metadata about the image, such as environment variables and entrypoint.
    async fn fetch_config(
        &self,
        repository: &str,
        digest: &Digest,
    ) -> MonocoreResult<ImageConfiguration>;

    /// Fetches a image blob from the registry by its digest.
    /// This method returns a stream for efficient processing of large blobs.
    ///
    /// `range` is the range of the blob to fetch, in bytes.
    /// If `range` is not provided, the entire blob is fetched.
    async fn fetch_image_blob(
        &self,
        repository: &str,
        digest: &Digest,
        range: impl RangeBounds<u64> + Send,
    ) -> MonocoreResult<BoxStream<'static, MonocoreResult<Bytes>>>;
}

/// Trait for pushing images to an OCI-compliant registry.
#[async_trait::async_trait]
pub trait OciRegistryPush {
    /// Uploads an OCI image to the specified repository.
    async fn upload_image(&self, image: &str) -> MonocoreResult<()>;
}

/// Trait for discovering tags in a repository.
#[async_trait::async_trait]
pub trait OciRegistryDiscovery {
    /// Fetches the tags available in a repository.
    async fn fetch_tags(&self, image: &str) -> MonocoreResult<Vec<String>>;
}

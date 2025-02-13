use std::path::PathBuf;

use getset::Getters;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use sqlx::{Pool, Sqlite};

use crate::{MonocoreError, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// SandboxesRegistry is a client for interacting with the Sandboxes Registry.
#[derive(Debug, Getters)]
pub struct SandboxesRegistry {
    /// The HTTP client used to make requests to the registry
    #[getset(get = "pub with_prefix")] // TODO: Remove
    client: ClientWithMiddleware,

    /// The directory where image data is stored
    #[getset(get = "pub with_prefix")]
    store_dir: PathBuf,

    /// The database connection pool
    #[getset(get = "pub with_prefix")]
    monoimage_db: Pool<Sqlite>,
}

impl SandboxesRegistry {
    /// Creates a new Sandboxes Registry client with the specified store directory and database.
    pub fn new(store_dir: PathBuf, monoimage_db: Pool<Sqlite>) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client_builder = ClientBuilder::new(Client::new());
        let client = client_builder
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            client,
            store_dir,
            monoimage_db,
        }
    }

    /// Pulls an image from the Sandboxes Registry.
    pub async fn pull_image(&self, _repository: &str, _reference: &str) -> MonocoreResult<()> {
        // For now, just return an error as specified
        Err(MonocoreError::NotImplemented(
            "Sandboxes Registry pull_image not yet implemented".to_string(),
        ))
    }
}

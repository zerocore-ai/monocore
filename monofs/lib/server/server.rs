use getset::Getters;
use nfsserve::tcp::{NFSTcp, NFSTcpListener};
use std::path::PathBuf;

use crate::store::FlatFsStore;

use super::MonofsNFS;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A server that provides NFS access to a content-addressed store.
/// This server uses a flat filesystem store as its backing store.
#[derive(Debug, Getters)]
#[getset(get = "pub with_prefix")]
pub struct MonofsServer {
    /// The path to the store.
    store_dir: PathBuf,

    /// The host to bind to.
    host: String,

    /// The port to listen on.
    port: u32,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MonofsServer {
    /// Creates a new MonofsServer with the given store path and host:port.
    pub fn new(store_dir: impl Into<PathBuf>, host: impl Into<String>, port: u32) -> Self {
        Self {
            store_dir: store_dir.into(),
            host: host.into(),
            port,
        }
    }

    /// Starts the NFS server and blocks until it is shut down.
    pub async fn start(&self) -> anyhow::Result<()> {
        // Create the store and NFS filesystem
        let store = FlatFsStore::new(&self.store_dir);
        let fs = MonofsNFS::new(store);

        // Create and start the NFS listener
        let addr = format!("{}:{}", self.host, self.port);
        let listener = NFSTcpListener::bind(&addr, fs).await?;
        listener.handle_forever().await?;

        Ok(())
    }
}

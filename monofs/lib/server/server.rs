use getset::Getters;
use monoutils_store::{FastCDCChunker, FlatLayout, MemoryStore};
use nfsserve::tcp::{NFSTcp, NFSTcpListener};
use std::path::PathBuf;

use crate::store::FlatFsStore;

use super::MonofsNFS;

/// A server that provides NFS access to a content-addressed store.
/// This server uses a flat filesystem store as its backing store.
#[derive(Debug, Getters)]
#[getset(get = "pub with_prefix")]
pub struct MonofsServer {
    /// The path to the store.
    store_path: PathBuf,

    /// The host to bind to.
    host: String,

    /// The port to listen on.
    port: u32,
}

impl MonofsServer {
    /// Creates a new MonofsServer with the given store path and host:port.
    pub fn new(store_path: impl Into<PathBuf>, host: impl Into<String>, port: u32) -> Self {
        Self {
            store_path: store_path.into(),
            host: host.into(),
            port,
        }
    }

    /// Starts the NFS server and blocks until it is shut down.
    pub async fn start(&self) -> anyhow::Result<()> {
        // Create the store and NFS filesystem
        let _store =
            FlatFsStore::<FastCDCChunker, FlatLayout>::new(&self.store_path.to_string_lossy());
        let store = MemoryStore::<FastCDCChunker, FlatLayout>::new();
        let fs = MonofsNFS::new(store);

        // Create and start the NFS listener
        let addr = format!("{}:{}", self.host, self.port);
        let listener = NFSTcpListener::bind(&addr, fs).await?;
        listener.handle_forever().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_monofsserver_creation() {
        let temp_dir = TempDir::new().unwrap();
        let server = MonofsServer::new(
            temp_dir.path().to_path_buf(),
            "127.0.0.1",
            0, // Use port 0 for testing
        );

        assert_eq!(server.store_path, temp_dir.path());
        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 0);
    }
}

use getset::Getters;
use nfsserve::tcp::{NFSTcp, NFSTcpListener};

use crate::VirtualFileSystem;

use super::VirtualFileSystemNFS;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A server that provides NFS access to a virtual filesystem.
/// This server can use any implementation of VirtualFileSystem as its backing store.
#[derive(Debug, Getters)]
#[getset(get = "pub with_prefix")]
pub struct VirtualFileSystemServer<F>
where
    F: VirtualFileSystem + Send + Sync + 'static,
{
    /// The virtual filesystem implementation to use.
    root: F,

    /// The host to bind to.
    host: String,

    /// The port to listen on.
    port: u32,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<F> VirtualFileSystemServer<F>
where
    F: VirtualFileSystem + Send + Sync + 'static,
{
    /// Creates a new VirtualFileSystemServer with the given virtual filesystem and host:port.
    pub fn new(root: F, host: impl Into<String>, port: u32) -> Self {
        Self {
            root,
            host: host.into(),
            port,
        }
    }

    /// Starts the NFS server and blocks until it is shut down.
    pub async fn start(self) -> anyhow::Result<()> {
        // Create the NFS filesystem wrapper
        let fs = VirtualFileSystemNFS::new(self.root);

        // Create and start the NFS listener
        let addr = format!("{}:{}", self.host, self.port);
        let listener = NFSTcpListener::bind(&addr, fs).await?;
        listener.handle_forever().await?;

        Ok(())
    }
}

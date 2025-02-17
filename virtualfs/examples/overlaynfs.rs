//! This example demonstrates running a simple NFS server using monofs.
//!
//! The example shows how to:
//! - Set up and configure an NFS server
//! - Serve a monofs filesystem over NFS
//! - Handle server configuration options
//!
//! Operations demonstrated:
//! 1. Parsing command line arguments for server configuration
//! 2. Setting up the NFS server
//! 3. Binding to a specified port
//! 4. Serving the filesystem
//!
//! To run the example:
//! ```bash
//! cargo run --example nfs -- /path/to/store --port 2049
//! ```

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use virtualfs::{MemoryFileSystem, VirtualFileSystemServer, DEFAULT_HOST, DEFAULT_NFS_PORT};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Simple NFS server that serves the monofs filesystem.
#[derive(Parser, Debug)]
#[command(author, long_about = None)]
struct Args {
    /// Paths to the layers to overlay
    layers: Vec<PathBuf>,

    /// Host address to bind to
    #[arg(short = 'H', long, default_value = DEFAULT_HOST)]
    host: String,

    /// Port to listen on
    #[arg(short = 'P', long, default_value_t = DEFAULT_NFS_PORT)]
    port: u32,
}

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create and start the server
    // let fs = OverlayFileSystem::new()?;
    let fs = MemoryFileSystem::new();
    let server = VirtualFileSystemServer::new(fs, args.host, args.port);
    tracing::info!(
        "Starting NFS server on {}:{}",
        server.get_host(),
        server.get_port()
    );

    server.start().await?;
    Ok(())
}

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
//! cargo run --example overlaynfs /path/to/layer1 /path/to/layer2 --host 127.0.0.1 --port 2049
//! ```

use anyhow::Result;
use clap::Parser;
use std::{net::IpAddr, path::PathBuf};
use virtualfs::{
    NativeFileSystem, OverlayFileSystem, VirtualFileSystem, VirtualFileSystemServer,
    DEFAULT_NFS_HOST, DEFAULT_NFS_PORT,
};

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
    #[arg(short = 'H', long, default_value = DEFAULT_NFS_HOST)]
    host: IpAddr,

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

    // Create filesystem layers from the provided paths
    let mut layers: Vec<Box<dyn VirtualFileSystem + Send + Sync>> = Vec::new();

    for path in &args.layers {
        let fs = Box::new(NativeFileSystem::new(path.clone()));
        layers.push(fs);
    }

    // Create and start the server
    let fs = OverlayFileSystem::new(layers)?;
    let server = VirtualFileSystemServer::new(fs, args.host.to_string(), args.port);
    tracing::info!(
        "starting NFS server on {}:{}",
        server.get_host(),
        server.get_port()
    );

    server.start().await?;
    Ok(())
}

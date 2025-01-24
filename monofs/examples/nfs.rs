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
//! cargo run --example nfs -- --path /path/to/store --port 2049
//! ```

use anyhow::Result;
use clap::Parser;
use monofs::server::MonofsServer;
use std::path::PathBuf;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Simple NFS server that serves the monofs filesystem.
#[derive(Parser, Debug)]
#[command(author, long_about = None)]
struct Args {
    /// Path to the store directory
    store_path: PathBuf,

    /// Host address to bind to
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 2049)]
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
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .init();

    // Create and start the server
    let server = MonofsServer::new(args.store_path, args.host, args.port);
    tracing::info!(
        "Starting NFS server on {}:{}",
        server.get_host(),
        server.get_port()
    );
    tracing::info!("Using store at: {}", server.get_store_path().display());

    server.start().await?;
    Ok(())
}

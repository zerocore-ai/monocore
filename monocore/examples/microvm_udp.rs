//! This example demonstrates UDP network connectivity between microvms using netcat (nc).
//! It creates two microvms - one running as a server and another as a client.
//!
//! To run in server mode (listens on port 3456):
//! ```bash
//! make example microvm_udp -- --server
//! # or
//! make example microvm_udp -- -s
//! ```
//!
//! To run in client mode (connects to localhost:3456):
//! ```bash
//! make example microvm_udp
//! ```
//!
//! You can specify a custom IP address using the --ip flag:
//! ```bash
//! make example microvm_udp -- --server --ip 192.168.1.100
//! make example microvm_udp -- --ip 192.168.1.100
//! ```
//!
//! To test the connection:
//! 1. Start the server in one terminal: `make example microvm_udp -- --server`
//! 2. Start the client in another terminal: `make example microvm_udp`
//!
//! The server will listen on UDP port 3456 and respond with "Hello from UDP server!" when it
//! receives a datagram. The client will send a datagram to the server, receive the response,
//! and both will exit after the interaction or after a timeout.
//!
//! By default, both server and client use 127.0.0.1 (localhost) as the IP address.
//! Use the --ip flag to specify a different IP address for either the server or client.

use anyhow::Result;
use clap::Parser;
use monocore::{
    utils,
    vm::{LogLevel, MicroVm},
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run as server (default is client)
    #[arg(short, long)]
    server: bool,

    /// IP address to use (default: 127.0.0.1)
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: String,
}

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Use specific directories for OCI and rootfs
    let build_dir = format!("{}/build", env!("CARGO_MANIFEST_DIR"));
    let oci_dir = format!("{}/oci", build_dir);

    // Parse image reference
    let image_ref = "library/alpine:latest";
    let (_, _, rootfs_name) = utils::parse_image_ref(image_ref).unwrap();
    let rootfs_dir = format!("{}/rootfs/reference/{}", build_dir, rootfs_name);

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, image_ref).await?;
    utils::merge_image_layers(&oci_dir, &rootfs_dir, image_ref).await?;

    // Build the MicroVm with different configurations based on server/client mode
    let vm = if args.server {
        tracing::info!("Server mode: Listening on {}:3456 (UDP)...", args.ip);
        MicroVm::builder()
            .log_level(LogLevel::Info)
            .root_path(format!("{}/merged", rootfs_dir))
            .port_map(["3456:3456".parse()?])
            .exec_path("/bin/busybox")
            .args([
                "timeout",
                "10",
                "busybox",
                "nc",
                "-u",                     // UDP mode
                "-l",                     // Listen mode
                "-v",                     // Verbose output for debugging
                "-p",                     // Port flag
                "3456",                   // Port to listen on
                "-e",                     // Execute following command
                "echo",                   // Echo command
                "Hello from UDP server!", // Message to send
            ])
            .assigned_ip(args.ip.parse()?)
            .ram_mib(512)
            .build()?
    } else {
        tracing::info!("Client mode: Connecting to {}:3456 (UDP)...", args.ip);
        MicroVm::builder()
            .log_level(LogLevel::Info)
            .root_path(format!("{}/merged", rootfs_dir))
            .exec_path("/bin/busybox")
            .args([
                "nc",        // netcat
                "-u",        // UDP mode
                "-w",        // Wait timeout
                "1",         // Wait 1 second before giving up
                "127.0.0.1", // IP address to connect to
                "3456",      // Port number to connect to
            ])
            .assigned_ip(args.ip.parse()?)
            .ram_mib(512)
            .build()?
    };

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

//! If you are trying to run this example, please make sure to run `make example microvm_tcp` from
//! the `monocore` subdirectory.
//!
//! This example demonstrates network connectivity between microvms using netcat (nc).
//! It creates two microvms - one running as a server and another as a client.
//!
//! To run in server mode (listens on port 3456):
//! ```bash
//! make example microvm_tcp -- --server
//! # or
//! make example microvm_tcp -- -s
//! ```
//!
//! To run in client mode (connects to localhost:3456):
//! ```bash
//! make example microvm_tcp
//! ```
//!
//! You can specify a custom IP address using the --ip flag:
//! ```bash
//! make example microvm_tcp -- --server --ip 192.168.1.100
//! make example microvm_tcp -- --ip 192.168.1.100
//! ```
//!
//! To test the connection:
//! 1. Start the server in one terminal: `make example microvm_tcp -- --server`
//! 2. Start the client in another terminal: `make example microvm_tcp`
//!
//! The server will listen on port 3456 and respond with "Hello from server!" when connected.
//! The client will connect to the server, receive the message, and both will exit after
//! the interaction or after a timeout.
//!
//! By default, both server and client use 127.0.0.1 (localhost) as the IP address.
//! Use the --ip flag to specify a different IP address for either the server or client.

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use anyhow::Result;
use clap::Parser;
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
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

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Use specific directories for OCI and rootfs
    let oci_dir = format!("{}/build/oci", env!("CARGO_MANIFEST_DIR"));
    let merge_dir = format!("{}/build/rootfs/alpine", env!("CARGO_MANIFEST_DIR"));

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, "library/alpine", "latest").await?;
    utils::merge_image_layers(&oci_dir, &merge_dir, "library/alpine", "latest").await?;

    let root_path = format!("{}/merged", merge_dir);

    // Build the MicroVm with different configurations based on server/client mode
    let vm = if args.server {
        tracing::info!("Server mode: Listening on {}:3456 (TCP)...", args.ip);
        MicroVm::builder()
            .log_level(LogLevel::Info)
            .root_path(root_path)
            .port_map(["3456:3456".parse()?])
            .exec_path("/bin/busybox")
            .args([
                "timeout",
                "10",
                "busybox",
                "nc",
                "-l",
                "-p",
                "3456",
                "-e",
                "echo",
                "Hello from TCP server!",
            ])
            .assigned_ip(args.ip.parse()?)
            .ram_mib(512)
            .build()?
    } else {
        tracing::info!("Client mode: Connecting to {}:3456 (TCP)...", args.ip);
        MicroVm::builder()
            .log_level(LogLevel::Info)
            .root_path(root_path)
            .exec_path("/bin/busybox")
            .args(["nc", "-w", "1", "127.0.0.1", "3456"])
            .assigned_ip(args.ip.parse()?)
            .ram_mib(512)
            .build()?
    };

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

#[cfg(target_os = "linux")] // TODO: Linux support temporarily on hold
fn main() {
    panic!("This example is not yet supported on Linux");
}

//! If you are trying to run this example, please make sure to run `make example microvm_nc` from
//! the `monocore` subdirectory.
//!
//! This example demonstrates network connectivity between microvms using netcat (nc).
//!
//! To run in server mode (listens on port 3456):
//! ```bash
//! make example microvm_nc -- --server
//! # or
//! make example microvm_nc -- -s
//! ```
//!
//! To run in client mode (connects to localhost:3456 and sends "Hello, world!"):
//! ```bash
//! make example microvm_nc
//! ```
//!
//! To test the connection:
//! 1. Start the server in one terminal: `make example microvm_nc -- --server`
//! 2. Start the client in another terminal: `make example microvm_nc`
//! The client will send "Hello, world!" to the server and then both will exit.

use anyhow::Result;
use clap::Parser;
use monocore::vm::MicroVm;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run as server (default is client)
    #[arg(short, long)]
    server: bool,
}

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Use the architecture-specific build directory
    let rootfs_path = format!("build/rootfs-alpine-{}", get_current_arch());

    // Build the MicroVm with different configurations based on server/client mode
    let vm = if args.server {
        tracing::info!("Server mode: Listening on port 3456");
        // Server mode: Listen on port 3456
        MicroVm::builder()
            .root_path(&rootfs_path)
            .port_map(["3456:3456".parse()?])
            .exec_path("/bin/busybox")
            .args(["timeout", "10", "busybox", "nc", "-l", "-p", "3456"])
            .ram_mib(1024)
            .build()?
    } else {
        tracing::info!("Client mode: Connecting to localhost:3456");
        // Client mode: Connect to localhost:3456 and send "Hello, world!"
        MicroVm::builder()
            .root_path(&rootfs_path)
            .exec_path("/bin/sh")
            .args(["-c", "echo 'Hello, world!' | busybox nc 127.0.0.1 3456"])
            .ram_mib(1024)
            .build()?
    };

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------

// Add this function to determine the current architecture
fn get_current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        panic!("Unsupported architecture")
    }
}

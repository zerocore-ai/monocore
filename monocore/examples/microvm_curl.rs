//! If you are trying to run this example, please make sure to run `make example microvm_curl` from
//! the `monocore` subdirectory.
//!
//! This example demonstrates making HTTP requests from inside a microvm using curl with:
//! - 1 virtual CPU
//! - 1024 MiB of RAM
//! - Configurable network restrictions
//! - Default target of example.com (93.184.216.34:80)
//!
//! To run the example with default settings (non-local mode):
//! ```bash
//! make example microvm_curl
//! ```
//!
//! To run with local-only network restrictions:
//! ```bash
//! make example microvm_curl -- --local-only
//! # or
//! make example microvm_curl -- -l
//! ```
//!
//! To specify a different target:
//! ```bash
//! make example microvm_curl -- localhost:8080
//! ```
//!
//! You can combine both options:
//! ```bash
//! make example microvm_curl -- --local-only localhost:8080
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use monocore::{utils, vm::MicroVm};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Target URL or IP:port to curl
    #[arg(default_value = "93.184.216.34:80")] // -> example.com
    target: String,

    /// Whether to restrict to local connections only
    #[arg(long, short, default_value_t = false)]
    local_only: bool,
}

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Parse command line arguments
    let args = Args::parse();

    // Use specific directories for OCI and rootfs
    let build_dir = format!("{}/build", env!("CARGO_MANIFEST_DIR"));
    let oci_dir = format!("{}/oci", build_dir);
    let rootfs_fedora_dir = format!("{}/rootfs/reference/library_fedora__latest", build_dir);

    // Pull and merge Fedora image
    utils::pull_docker_image(&oci_dir, "library/fedora:latest").await?;
    utils::merge_image_layers(&oci_dir, &rootfs_fedora_dir, "library/fedora:latest").await?;

    // Build the MicroVm
    let vm = MicroVm::builder()
        .root_path(format!("{}/merged", rootfs_fedora_dir))
        .num_vcpus(1)
        .exec_path("/bin/curl")
        .args([args.target.as_str()])
        .local_only(args.local_only)
        .ram_mib(1024)
        .build()
        .context("Failed to build MicroVm")?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

//! If you are trying to run this example, please make sure to run `make example microvm_shell` from
//! the `monocore` subdirectory.
//!
//! This example demonstrates running a basic shell inside a microvm with:
//! - 2 virtual CPUs
//! - 1024 MiB of RAM
//! - Basic environment setup (PATH=/bin)
//! - Resource limits (RLIMIT_NOFILE set to 256:512)
//!
//! To run the example:
//! ```bash
//! make example microvm_shell
//! ```
//!
//! Once running, you can interact with the shell inside the microvm.
//! The shell has basic functionality and access to busybox commands.

use anyhow::{Context, Result};
use monocore::{
    utils,
    vm::{LogLevel, MicroVm},
};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Use specific directories for OCI and rootfs
    let oci_dir = format!("{}/build/oci", env!("CARGO_MANIFEST_DIR"));
    let merge_dir = format!("{}/build/rootfs/alpine", env!("CARGO_MANIFEST_DIR"));

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, "library/alpine", "latest").await?;
    utils::merge_image_layers(&oci_dir, &merge_dir, "library/alpine", "latest").await?;

    // Build the MicroVm
    let vm = MicroVm::builder()
        .log_level(LogLevel::Info)
        .root_path(format!("{}/merged", merge_dir))
        .num_vcpus(2)
        .exec_path("/bin/sh")
        .rlimits(["RLIMIT_NOFILE=256:512".parse()?])
        .env(["PATH=/bin".parse()?])
        .ram_mib(1024)
        .port_map(["8080:8080".parse()?])
        .local_only(false)
        .build()
        .context("Failed to build MicroVm")?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

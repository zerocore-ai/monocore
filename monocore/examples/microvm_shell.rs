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
    let build_dir = format!("{}/build", env!("CARGO_MANIFEST_DIR"));
    let oci_dir = format!("{}/oci", build_dir);

    // Parse image reference
    let image_ref = "library/alpine:latest";
    let (_, _, rootfs_name) = utils::parse_image_ref(image_ref).unwrap();
    let rootfs_dir = format!("{}/rootfs/reference/{}", build_dir, rootfs_name);

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, image_ref).await?;
    utils::merge_image_layers(&oci_dir, &rootfs_dir, image_ref).await?;

    // Build the MicroVm
    let merged_rootfs_dir = format!("{}/merged", rootfs_dir);
    let vm = MicroVm::builder()
        .log_level(LogLevel::Info)
        .root_path(merged_rootfs_dir)
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

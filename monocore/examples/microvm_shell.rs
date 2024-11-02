//! If you are trying to run this example, please make sure to run `make example microvm_shell` from
//! the `monocore` subdirectory

use anyhow::{Context, Result};
use monocore::vm::MicroVm;
use std::path::Path;

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Use the architecture-specific build directory
    let rootfs_path = format!("build/rootfs-alpine-{}", get_current_arch());

    // Check if rootfs exists
    if !Path::new(&rootfs_path).exists() {
        anyhow::bail!(
            "Rootfs directory '{}' does not exist. Please run 'make unpack_rootfs' first.",
            rootfs_path
        );
    }

    // Update the set_xattr call
    set_xattr(&rootfs_path, "user.containers.override_stat", b"0:0:0555")
        .context("Failed to set xattr on rootfs")?;

    // Build the MicroVm
    let vm = MicroVm::builder()
        .root_path(&rootfs_path)
        .num_vcpus(2)
        .exec_path("/bin/sh")
        .rlimits(["RLIMIT_NOFILE=256:512".parse()?])
        .env(["PATH=/bin".parse()?])
        .ram_mib(1024)
        .build()
        .context("Failed to build MicroVm")?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Function: *
//--------------------------------------------------------------------------------------------------

// Set an extended attribute on a file
fn set_xattr(path: impl AsRef<std::path::Path>, name: &str, value: &[u8]) -> anyhow::Result<()> {
    xattr::set(path, name, value)?;
    Ok(())
}

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

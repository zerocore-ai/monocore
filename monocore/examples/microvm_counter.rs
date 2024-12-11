//! This example demonstrates running a counter program inside a microvm that randomly chooses between:
//! - A Python implementation using python:3.11-slim image
//! - A Shell implementation using alpine:latest image
//!
//! Both implementations:
//! - Count from 1 to 10
//! - Sleep 2 seconds between counts
//! - Use 256 MiB of RAM
//!
//! To run the example:
//! ```bash
//! make example microvm_counter
//! ```
//!
//! The program will randomly choose between Python and Shell implementations
//! and display the counting progress in the terminal.

use monocore::{utils, vm::MicroVm};
use rand::Rng;

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Use specific directories for OCI and rootfs
    let build_dir = format!("{}/build", env!("CARGO_MANIFEST_DIR"));
    let oci_dir = format!("{}/oci", build_dir);

    // Randomly choose between Python and Shell
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.5) {
        println!("Running Python counter...");
        run_python(&build_dir, &oci_dir).await?;
    } else {
        println!("Running Shell counter...");
        run_sh(&build_dir, &oci_dir).await?;
    }

    Ok(())
}

async fn run_python(build_dir: &str, oci_dir: &str) -> anyhow::Result<()> {
    let image_ref = "library/python:3.11-slim";
    let (_, _, rootfs_name) = utils::parse_image_ref(image_ref).unwrap();
    let rootfs_dir = format!("{}/rootfs/reference/{}", build_dir, rootfs_name);

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, image_ref).await?;
    utils::merge_image_layers(&oci_dir, &rootfs_dir, image_ref).await?;

    // Build the MicroVm
    let vm = MicroVm::builder()
        .root_path(format!("{}/merged", rootfs_dir))
        .exec_path("/usr/local/bin/python3")
        .args(["-c", "import time; count=0; [print(f'Count: {count+1}') or time.sleep(2) or (count:=count+1) for _ in range(10)]"])
        .ram_mib(256)
        .build()?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

async fn run_sh(build_dir: &str, oci_dir: &str) -> anyhow::Result<()> {
    let image_ref = "library/alpine:latest";
    let (_, _, rootfs_name) = utils::parse_image_ref(image_ref).unwrap();
    let rootfs_dir = format!("{}/rootfs/reference/{}", build_dir, rootfs_name);

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, image_ref).await?;
    utils::merge_image_layers(&oci_dir, &rootfs_dir, image_ref).await?;

    // Build the MicroVm
    let vm = MicroVm::builder()
        .root_path(format!("{}/merged", rootfs_dir))
        .exec_path("/bin/sh")
        .args(["-c", "for i in $(seq 1 10); do echo $i; sleep 2; done"])
        .ram_mib(256)
        .build()?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

//! To run this example:
//! ```bash
//! make example microvm_nop
//! ```

use monocore::{utils, vm::MicroVm};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    let vm = MicroVm::builder()
        .root_path(format!("{}/merged", rootfs_dir))
        .exec_path("/bin/true")
        .ram_mib(1024)
        .build()?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

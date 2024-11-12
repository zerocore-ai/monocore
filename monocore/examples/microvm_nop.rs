//! If you are trying to run this example, please make sure to run `make example microvm_nop` from
//! the `monocore` subdirectory

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use monocore::{utils, vm::MicroVm};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Use specific directories for OCI and rootfs
    let oci_dir = format!("{}/build/oci", env!("CARGO_MANIFEST_DIR"));
    let merge_dir = format!("{}/build/rootfs/alpine", env!("CARGO_MANIFEST_DIR"));

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, "library/alpine", "latest").await?;
    utils::merge_image_layers(&oci_dir, &merge_dir, "library/alpine", "latest").await?;

    // Build the MicroVm
    let vm = MicroVm::builder()
        .root_path(format!("{}/merged", merge_dir))
        .exec_path("/bin/true")
        .ram_mib(1024)
        .build()?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;

    Ok(())
}

#[cfg(target_os = "linux")] // TODO: Linux support temporarily on hold
fn main() {
    panic!("This example is not yet supported on Linux");
}

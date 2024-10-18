//! If you are trying to run this example, please make sure to run `make example microvm_shell` from
//! the `monocore` subdirectory

use monocore::runtime::MicroVM;

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    // Get the current architecture
    let arch = get_current_arch();

    // Use the architecture-specific build directory
    let rootfs_path = format!("build/rootfs-alpine-{}", arch);

    // Update the set_xattr call // TODO: Not sure how important this is
    set_xattr(&rootfs_path, "user.containers.override_stat", b"0:0:0555")?;

    // Build the microVM
    let vm = MicroVM::builder()
        .root_path(&rootfs_path)
        .num_vcpus(2)
        .exec_path("/bin/sh")
        .rlimits(["RLIMIT_NOFILE=256:512".parse()?])
        .env(["ALIEN_GREETING=Hello puny humans!".parse()?])
        .ram_mib(1024)
        .build()?;

    // Start the microVM
    vm.start();

    Ok(())
}

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

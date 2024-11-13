//! This example demonstrates how to merge OCI image layers using OverlayFsMerger.
//!
//! The example will:
//! 1. Create a Docker registry client
//! 2. Pull the Node.js image (which has multiple layers)
//! 3. Merge the layers using OverlayFsMerger
//! 4. Show where the merged rootfs is stored and examine key directories
//!
//! To run the example:
//! ```bash
//! cargo run --example overlayfs_merge
//! ```

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use monocore::oci::rootfs;
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use monocore::utils::{self, OCI_LAYER_SUBDIR};
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use std::path::Path;
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use tempfile::tempdir;
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
use walkdir::WalkDir;

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let root_path = tempdir()?;

    // Use specific directories for OCI and rootfs
    let oci_dir = root_path.path().join("oci");
    let rootfs_node_dir = root_path.path().join("rootfs/reference/library_node__slim");

    // Pull node image
    utils::pull_docker_image(&oci_dir, "library/node:slim").await?;

    // Show the layer structure before merging
    print_layer_structure(&oci_dir)?;

    // Create destination directory for merged layers
    std::fs::create_dir_all(&rootfs_node_dir)?;

    // Merge the layers
    println!("\nMerging layers...");
    rootfs::merge(oci_dir, &rootfs_node_dir, "library_node__slim").await?;

    // Show the merged rootfs structure focusing on interesting directories
    print_ref_rootfs(&rootfs_node_dir)?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------

// Helper function to print the layer structure before merging
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
fn print_layer_structure(base_path: impl AsRef<Path>) -> anyhow::Result<()> {
    let layers_dir = base_path.as_ref().join(OCI_LAYER_SUBDIR);

    println!("\nLayer Structure Before Merge:");
    println!("----------------------------");
    println!("Layers directory: {}", layers_dir.display());

    let mut layer_count = 0;
    for entry in std::fs::read_dir(&layers_dir)? {
        let entry = entry?;
        layer_count += 1;
        println!(
            "Layer {}: {}",
            layer_count,
            entry.file_name().to_string_lossy()
        );
    }

    println!("\nTotal layers: {}", layer_count);
    Ok(())
}

// Helper function to print the merged rootfs directory structure
#[cfg(all(unix, not(target_os = "linux")))] // TODO: Linux support temporarily on hold
fn print_ref_rootfs(rootfs_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let rootfs_dir = rootfs_dir.as_ref().join("merged");
    println!("\nMerged Rootfs Structure:");
    println!("------------------------");
    println!("Root: {}", rootfs_dir.display());

    // List of interesting directories to examine in the Node.js image
    let interesting_dirs = vec![
        "bin",                        // Shows basic binaries
        "usr/local/bin",              // Shows Node.js binaries
        "usr/local/lib/node_modules", // Shows npm packages
        "etc",                        // Shows configuration files
    ];

    for dir in interesting_dirs {
        let path = rootfs_dir.join(dir);
        if path.exists() {
            println!("\nContents of /{}/:", dir);
            for entry in WalkDir::new(&path)
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                println!("  {}", entry.file_name().to_string_lossy());
            }
        }
    }

    println!("\nNote: These files are in a temporary directory and will be deleted when the program exits.");
    println!("      For persistent storage, use paths in your project directory.");
    println!("\nThe Node.js image demonstrates overlayfs by showing how multiple layers");
    println!("(base OS, Node.js runtime, npm packages) are combined into a single filesystem.");

    Ok(())
}

#[cfg(target_os = "linux")] // TODO: Linux support temporarily on hold
fn main() {
    panic!("This example is not yet supported on Linux");
}

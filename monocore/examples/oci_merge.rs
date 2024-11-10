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

use monocore::{
    oci::{
        distribution::{DockerRegistry, OciRegistryPull},
        overlayfs::OverlayFsMerger,
    },
    utils::OCI_LAYER_SUBDIR,
};
use std::path::PathBuf;
use walkdir::WalkDir;

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create OCI download directory for this example
    let oci_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build");

    // Create Docker registry client.
    let registry = DockerRegistry::with_oci_dir(oci_dir.clone());

    // Pull Node.js image (has multiple layers showing overlayfs in action)
    println!("\nPulling Node.js image...");
    registry.pull_image("library/node", Some("slim")).await?;

    // Show the layer structure before merging
    print_layer_structure(registry.get_oci_dir())?;

    // Create destination directory for merged layers
    let merged_rootfs_dir = oci_dir.join("merged_rootfs");
    std::fs::create_dir_all(&merged_rootfs_dir)?;

    // Create OverlayFsMerger instance
    println!("\nMerging layers...");
    let merger = OverlayFsMerger::new(oci_dir, merged_rootfs_dir.clone());

    // Merge the layers
    merger.merge("library_node__slim").await?;

    // Show the merged rootfs structure focusing on interesting directories
    print_merged_rootfs(&merged_rootfs_dir.join("merged"))?;

    // // Cleanup
    // merger.unmount().await?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------

// Helper function to print the layer structure before merging
fn print_layer_structure(base_path: &std::path::Path) -> anyhow::Result<()> {
    let layers_dir = base_path.join(OCI_LAYER_SUBDIR);

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
fn print_merged_rootfs(merged_dir: &PathBuf) -> anyhow::Result<()> {
    println!("\nMerged Rootfs Structure:");
    println!("------------------------");
    println!("Root: {}", merged_dir.display());

    // List of interesting directories to examine in the Node.js image
    let interesting_dirs = vec![
        "bin",                        // Shows basic binaries
        "usr/local/bin",              // Shows Node.js binaries
        "usr/local/lib/node_modules", // Shows npm packages
        "etc",                        // Shows configuration files
    ];

    for dir in interesting_dirs {
        let path = merged_dir.join(dir);
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

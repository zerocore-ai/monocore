//! This example demonstrates how to pull OCI images from Docker Hub using the Docker registry implementation.
//!
//! The example will:
//! 1. Create a Docker registry client
//! 2. Pull the Alpine Linux image
//! 3. Show where the downloaded files are stored
//!
//! To run the example:
//! ```bash
//! make example oci_pull
//! ```

use monocore::{
    oci::distribution::{DockerRegistry, OciRegistryPull},
    utils::{OCI_LAYER_SUBDIR, OCI_REPO_SUBDIR},
};
use std::path::PathBuf;
use tempfile::tempdir;

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create a temporary directory for this example
    let temp_dir = tempdir()?;
    println!("\nUsing temporary directory: {}", temp_dir.path().display());

    // Create Docker registry client
    let registry = DockerRegistry::with_oci_dir(temp_dir.path().to_path_buf());

    // Pull Alpine Linux image
    println!("\nPulling Alpine Linux image...");
    registry
        .pull_image("library/alpine", Some("latest"))
        .await?;

    // Show the downloaded files
    print_oci_files(temp_dir.path().to_path_buf())?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------

// Helper function to print the OCI directory structure and files
fn print_oci_files(oci_dir: PathBuf) -> anyhow::Result<()> {
    println!("\nOCI Directory Structure:");
    println!("------------------------");

    // Print repo directory contents
    let repo_dir = oci_dir.join(OCI_REPO_SUBDIR).join("library_alpine__latest");
    println!("\nRepository files at {}:", repo_dir.display());
    for entry in std::fs::read_dir(&repo_dir)? {
        let entry = entry?;
        println!("- {}", entry.file_name().to_string_lossy());
    }

    // Print layers directory contents
    let layers_dir = oci_dir.join(OCI_LAYER_SUBDIR);
    println!("\nLayers at {}:", layers_dir.display());
    for entry in std::fs::read_dir(layers_dir)? {
        let entry = entry?;
        println!("- {}", entry.file_name().to_string_lossy());
    }

    println!("\nNote: These files are in a temporary directory and will be deleted when the program exits.");
    println!(
        "      For persistent storage, use DockerRegistry::new() which stores files in ~/.monocore"
    );

    Ok(())
}

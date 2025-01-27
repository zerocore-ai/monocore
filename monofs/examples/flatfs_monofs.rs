//! This example demonstrates a simple filesystem using FlatFsStore for persistence.
//!
//! The example shows how to:
//! - Create a filesystem with persistent storage
//! - Perform basic filesystem operations
//! - Work with directories and files
//! - Handle content persistence using CIDs
//!
//! Operations demonstrated:
//! 1. Setting up a persistent filesystem
//! 2. Creating directory structures
//! 3. Creating and writing to files
//! 4. Copying and moving files
//! 5. Listing directory contents
//! 6. Managing filesystem state with CIDs
//!
//! To run the example:
//! ```bash
//! cargo run --example flatfs_monofs -- /path/to/filesystem
//! ```

use anyhow::Result;
use clap::Parser;
use monofs::{
    filesystem::{Dir, Entity, File},
    store::FlatFsStoreDefault,
};
use monoutils_store::{ipld::cid::Cid, IpldStore, Storable};
use std::future::Future;
use std::pin::Pin;
use tokio::io::AsyncWriteExt;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Example demonstrating a simple filesystem with persistence
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to the filesystem directory
    #[arg(
        help = "Path to the filesystem directory. Will be created if it doesn't exist. Root directory CID stored in $path/head"
    )]
    path: std::path::PathBuf,
}

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create filesystem directory and blocks subdirectory
    let blocks_path = args.path.join("blocks");
    tokio::fs::create_dir_all(&blocks_path).await?;
    println!("\nUsing filesystem directory: {}\n", args.path.display());

    // Initialize the store with blocks directory
    let store = FlatFsStoreDefault::new(blocks_path.to_str().unwrap());

    // Path to head CID file
    let head_path = args.path.join("head");

    // Try to read existing head CID
    let head_cid = if head_path.exists() {
        let head_contents = tokio::fs::read_to_string(&head_path).await?;
        Some(Cid::try_from(head_contents.trim())?)
    } else {
        None
    };

    // Load or create root directory and perform operations
    let root = if let Some(cid) = head_cid {
        println!("Loading existing filesystem from CID: {}", cid);
        Dir::load(&cid, store.clone()).await?
    } else {
        println!("Creating new filesystem...");
        let mut root = Dir::new(store.clone());
        perform_example_operations(&mut root).await?;

        // Store the root directory and save its CID
        let root_cid = root.store().await?;
        tokio::fs::write(&head_path, root_cid.to_string()).await?;
        println!("\nStored new filesystem with root CID: {}", root_cid);

        root
    };

    // Always display the current filesystem structure
    println!("\nFilesystem contents:");
    print_dir_contents(&root, 0).await?;

    Ok(())
}

/// Performs example filesystem operations to demonstrate functionality.
/// This function is only called when creating a new filesystem (no existing head CID).
/// It creates a sample directory structure with files and demonstrates basic operations.
async fn perform_example_operations<S: IpldStore + Send + Sync + 'static>(
    root: &mut Dir<S>,
) -> Result<()> {
    println!("\nDemonstrating filesystem operations:");

    // 1. Create some directories
    println!("\n1. Creating directories...");
    root.find_or_create("docs/guides", false).await?;
    root.find_or_create("projects/rust", false).await?;
    root.find_or_create("data/configs", false).await?;

    // 2. Create and write to some files
    println!("\n2. Creating and writing to files...");

    // Create a README in docs
    let mut readme = File::new(root.get_store().clone());
    let mut output = readme.get_output_stream();
    output
        .write_all(b"# Documentation\n\nWelcome to the docs!")
        .await?;
    output.flush().await?;
    drop(output);

    let docs = root.find_mut("docs").await?.unwrap();
    if let Entity::Dir(ref mut docs_dir) = docs {
        docs_dir.put_adapted_file("README.md", readme).await?;
    }

    // Create a config file
    let mut config = File::new(root.get_store().clone());
    let mut output = config.get_output_stream();
    output
        .write_all(b"{\n  \"version\": \"1.0.0\",\n  \"name\": \"flat-monofs\"\n}")
        .await?;
    output.flush().await?;
    drop(output);

    let configs = root.find_or_create("data/configs", false).await?;
    if let Entity::Dir(ref mut configs_dir) = configs {
        configs_dir.put_adapted_file("app.json", config).await?;
    }

    // Create a Rust project file
    let mut main_rs = File::new(root.get_store().clone());
    let mut output = main_rs.get_output_stream();
    output
        .write_all(b"fn main() {\n    println!(\"Hello from flat-monofs!\");\n}")
        .await?;
    output.flush().await?;
    drop(output);

    let rust = root.find_or_create("projects/rust", false).await?;
    if let Entity::Dir(ref mut rust_dir) = rust {
        rust_dir.put_adapted_file("main.rs", main_rs).await?;
    }

    // 3. List contents recursively
    println!("\n3. Listing filesystem contents:");
    print_dir_contents(root, 0).await?;

    // 4. Demonstrate file operations
    println!("\n4. Demonstrating file operations:");

    // Copy a file
    root.copy("docs/README.md", "projects").await?;
    println!("Copied docs/README.md to projects/");

    // Move a file
    root.rename("data/configs/app.json", "projects/config.json")
        .await?;
    println!("Moved data/configs/app.json to projects/config.json");

    // Remove a file
    root.remove("projects/rust/main.rs").await?;
    println!("Removed projects/rust/main.rs");

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Recursively prints the contents of a directory with proper indentation
fn print_dir_contents<S: IpldStore + Send + Sync + 'static>(
    dir: &Dir<S>,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
    Box::pin(async move {
        let indent = "  ".repeat(depth);

        for (name, link) in dir.get_entries() {
            let entity = link.resolve_entity(dir.get_store().clone()).await?;

            match entity {
                Entity::Dir(subdir) => {
                    println!("{}📁 {}/", indent, name);
                    print_dir_contents(&subdir, depth + 1).await?;
                }
                Entity::File(file) => {
                    let size = file.get_size().await?;
                    println!("{}📄 {} ({} bytes)", indent, name, size);
                }
                _ => println!("{}🔗 {}", indent, name),
            }
        }

        Ok(())
    })
}

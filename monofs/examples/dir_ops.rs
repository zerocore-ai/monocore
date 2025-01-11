//! This example demonstrates directory operations in monofs using an in-memory store.
//!
//! The example shows how to:
//! - Create directories and files
//! - Navigate directory structures
//! - Copy and move files
//! - List directory contents
//! - Store and load directories from CIDs
//!
//! Operations demonstrated:
//! 1. Creating root directory
//! 2. Creating nested files and directories
//! 3. Listing directory contents
//! 4. Copying files between directories
//! 5. Removing files and directories
//! 6. Working with subdirectories
//! 7. Checking entry existence
//! 8. Storing and loading directories using CIDs
//!
//! To run the example:
//! ```bash
//! cargo run --example dir_ops
//! ```

use monofs::filesystem::{Dir, File, FsResult};
use monoutils_store::{MemoryStore, Storable};

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> FsResult<()> {
    // Create a new MemoryStore
    let store = MemoryStore::default();

    // Create a new root directory
    let mut root = Dir::new(store.clone());
    println!("Created root directory: {:?}", root);

    // Find or create a file
    let file = root.find_or_create("docs/readme.md", true).await?;
    println!("Created file: {:?}", file);

    // Find or create a directory
    let dir = root.find_or_create("projects/rust", false).await?;
    println!("Created directory: {:?}", dir);

    // List contents of root directory
    let entries = root.list()?;
    println!("Root directory contents: {:?}", entries);

    // Copy a file
    root.copy("docs/readme.md", "projects").await?;
    println!("Copied 'readme.md' to 'projects' directory");

    // Find the copied file
    let copied_file = root.find("projects/readme.md").await?;
    println!("Copied file: {:?}", copied_file);

    // Remove a file
    root.remove("docs/readme.md").await?;
    println!("Removed 'docs/readme.md'");

    // Create and add a subdirectory
    root.put_dir("subdir", Dir::new(store.clone()))?;
    println!("Added subdirectory 'subdir'");

    // Create and add a file to the root directory
    root.put_file("example.txt", File::new(store.clone()))?;
    println!("Added file 'example.txt' to root");

    // List entries in the root directory
    println!("Entries in root directory:");
    for (name, entity) in root.get_entries() {
        println!("- {}: {:?}", name, entity);
    }

    // Check if an entry exists
    let file_exists = root.has_entry("example.txt")?;
    println!("'example.txt' exists: {}", file_exists);

    // Get and modify a subdirectory
    if let Some(subdir) = root.get_dir_mut("subdir").await? {
        subdir.put_file("subdir_file.txt", File::new(store.clone()))?;
        println!("Added 'subdir_file.txt' to 'subdir'");
    }

    // Remove an entry
    root.remove_entry("example.txt")?;
    println!("Removed 'example.txt' from root");

    // Check if the directory is empty
    println!("Root directory is empty: {}", root.is_empty());

    // Store the root directory
    let root_cid = root.store().await?;
    println!("Stored root directory with CID: {}", root_cid);

    // Load the root directory
    let loaded_root = Dir::load(&root_cid, store).await?;
    println!("Loaded root directory: {:?}", loaded_root);

    Ok(())
}

//! This example demonstrates file operations in monofs using an in-memory store.
//!
//! The example shows how to:
//! - Create and manipulate files
//! - Read and write file content
//! - Work with file streams
//! - Handle file metadata
//! - Store and load files from CIDs
//!
//! Operations demonstrated:
//! 1. Creating new files
//! 2. Writing content using FileOutputStream
//! 3. Reading content using FileInputStream and BufReader
//! 4. Checking file status (empty/size)
//! 5. Working with file metadata
//! 6. Storing and loading files using CIDs
//! 7. Truncating files
//!
//! To run the example:
//! ```bash
//! cargo run --example file
//! ```

use ipldstore::{MemoryStore, Storable};
use monofs::filesystem::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create a new MemoryStore
    let store = MemoryStore::default();

    // Create a new file
    let mut file = File::new(store.clone());
    println!("Created new file: {:?}", file);

    // Write content to the file
    let content = b"Hello, monofs!";
    let mut output_stream = file.get_output_stream();
    output_stream.write_all(content).await?;
    output_stream.flush().await?;
    drop(output_stream);
    println!("Wrote content to file");

    // Read content from the file
    let input_stream = file.get_input_stream().await?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(input_stream);
    reader.read_to_end(&mut buffer).await?;
    println!(
        "Read content from file: {}",
        String::from_utf8_lossy(&buffer)
    );

    // Check if the file is empty
    println!("File is empty: {}", file.is_empty().await?);

    // Get and print file metadata
    let metadata = file.get_metadata();
    println!("File metadata: {:?}", metadata);

    // Store the file
    let file_cid = file.store().await?;
    println!("Stored file with CID: {}", file_cid);

    // Load the file
    let loaded_file = File::load(&file_cid, store).await?;
    println!("Loaded file: {:?}", loaded_file);

    // Drop reader to free up reference to the file
    drop(reader);

    // Truncate the file
    file.truncate();

    println!("Truncated file");
    println!("File is empty after truncation: {}", file.is_empty().await?);

    Ok(())
}

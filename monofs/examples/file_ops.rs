use monofs::filesystem::{File, FileInputStream, FileOutputStream};
use monoutils_store::{MemoryStore, Storable};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a new MemoryStore
    let store = MemoryStore::default();

    // Create a new file
    let mut file = File::new(store.clone());
    println!("Created new file: {:?}", file);

    // Write content to the file
    let content = b"Hello, monofs!";
    let mut output_stream = FileOutputStream::new(&mut file);
    output_stream.write_all(content).await?;
    output_stream.shutdown().await?;
    println!("Wrote content to file");

    // Read content from the file
    let input_stream = FileInputStream::new(&file).await?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(input_stream);
    reader.read_to_end(&mut buffer).await?;
    println!(
        "Read content from file: {}",
        String::from_utf8_lossy(&buffer)
    );
    drop(reader); // Drop reader to free up the input stream ref to the file

    // Check if the file is empty
    println!("File is empty: {}", file.is_empty());

    // Get and print file metadata
    let metadata = file.get_metadata();
    println!("File metadata: {:?}", metadata);

    // Store the file
    let file_cid = file.store().await?;
    println!("Stored file with CID: {}", file_cid);

    // Load the file
    let loaded_file = File::load(&file_cid, store).await?;
    println!("Loaded file: {:?}", loaded_file);

    // Truncate the file
    file.truncate();
    println!("Truncated file");
    println!("File is empty after truncation: {}", file.is_empty());

    Ok(())
}

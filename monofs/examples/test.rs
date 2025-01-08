use monofs::filesystem::{Dir, File, FsResult};
use monoutils_store::{MemoryStore, Storable};

#[tokio::main]
async fn main() -> FsResult<()> {
    let store = MemoryStore::default();

    // Create a new root directory
    let mut root = Dir::new(store.clone());

    // Create a file in the directory
    root.put_file("example.txt", File::new(store.clone()))?;

    // Create a subdirectory
    root.put_dir("subdir", Dir::new(store.clone()))?;

    // List directory contents
    for (name, entity) in root.get_entries() {
        println!("- {}: {:?}", name, entity);
    }

    // Store the directory
    let root_cid = root.store().await?;
    println!("Stored root directory with CID: {}", root_cid);

    Ok(())
}

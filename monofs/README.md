<div align="center">
  <h1 align="center">monofs</h1>

  <p>
    <a href="https://github.com/appcypher/monocore/actions?query=">
      <img src="https://github.com/appcypher/monocore/actions/workflows/tests_and_checks.yml/badge.svg" alt="Build Status">
    </a>
    <a href="https://github.com/appcypher/monocore/blob/main/LICENSE">
      <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License">
    </a>
  </p>
</div>

**`monofs`** is a powerful, distributed filesystem designed for distributed workloads. It provides a simple and intuitive API for managing files and directories in a content-addressed storage system.

> [!WARNING]
> This project is in early development and is not yet ready for production use.

##

## Features

- Content-addressed storage
- Immutable data structures with copy-on-write semantics
- Support for files, directories, and symbolic links
- Asynchronous API for efficient I/O operations
- Versioning support for tracking file and directory history

## Usage

Here are some examples of how to use the `monofs` API:

### Working with Files

```rust
use monofs::filesystem::{File, FileInputStream, FileOutputStream};
use monoutils_store::{MemoryStore, Storable};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = MemoryStore::default();

    // Create a new file
    let mut file = File::new(store.clone());

    // Write content to the file
    let mut output_stream = FileOutputStream::new(&mut file);
    output_stream.write_all(b"Hello, monofs!").await?;
    output_stream.shutdown().await?;

    // Read content from the file
    let input_stream = FileInputStream::new(&file).await?;
    let mut buffer = Vec::new();
    input_stream.read_to_end(&mut buffer).await?;

    println!("File content: {}", String::from_utf8_lossy(&buffer));

    // Store the file
    let file_cid = file.store().await?;
    println!("Stored file with CID: {}", file_cid);

    Ok(())
}
```

### Working with Directories

```rust
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
```

## API Overview

- `File`: Represents a file in the filesystem
- `Dir`: Represents a directory in the filesystem
- `FileInputStream`: Provides read access to file contents
- `FileOutputStream`: Provides write access to file contents
- `Metadata`: Stores metadata for files and directories
- `Storable`: Trait for storing and loading entities from the content-addressed store

For more detailed examples and API usage, check out the `examples` directory and the API documentation.

## Development

To set up `monofs` for development:

1. Ensure you have Rust installed (latest stable version)
2. Clone the monocore repository:
   ```sh
   git clone https://github.com/appcypher/monocore
   cd monocore/monofs
   ```
3. Build the project:
   ```sh
   cargo build
   ```
4. Run tests:
   ```sh
   cargo test
   ```

## Contributing

Contributions are welcome! Please read the [CONTRIBUTING.md](../CONTRIBUTING.md) file for guidelines on how to contribute to this project.

## License

This project is licensed under the [Apache License 2.0](../LICENSE).

## Acknowledgements

monofs draws inspiration from the [WNFS (Webnative File System)](https://github.com/wnfs-wg/rs-wnfs) project.

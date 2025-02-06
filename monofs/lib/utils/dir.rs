//! Directory utility functions for monofs.
//!
//! This module provides utilities for working with directories in the monofs filesystem,
//! including functions for displaying directory structures in a tree-like format.

use async_recursion::async_recursion;
use monoutils_store::IpldStore;

use crate::{filesystem::Dir, FsResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Number of spaces used for each level of indentation
const INDENT_SIZE: usize = 4;

/// Vertical line character for tree drawing
const VERTICAL: &str = "│";

/// Branch character for tree drawing (non-last items)
const BRANCH: &str = "├──";

/// Leaf character for tree drawing (last items)
const LEAF: &str = "└──";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Prints a visual tree representation of the directory structure.
///
/// This function displays the entire directory hierarchy using traditional tree-drawing characters
/// and visual indicators for different types of entries. The output follows common filesystem
/// conventions for ordering:
///
/// 1. Directories first (with 📁 icon and trailing slash)
/// 2. Files next (with 📄 icon and size in bytes)
/// 3. Symbolic links last (with 🔗 icon)
///
/// Within each category (directories/files/symlinks), entries are sorted alphabetically.
///
/// ## Examples
///
/// ```
/// use monofs::{filesystem::Dir, utils};
/// use monoutils_store::MemoryStore;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let store = MemoryStore::default();
/// let mut dir = Dir::new(store.clone());
///
/// // Create some test files and directories
/// dir.find_or_create("docs/README.md", true).await?;
/// dir.find_or_create("src/main.rs", true).await?;
/// dir.find_or_create("src/lib/mod.rs", true).await?;
///
/// // Print the directory tree
/// utils::print_dir_tree(&dir).await?;
/// // Output will look like:
/// // ├── 📁 docs/
/// // │   └── 📄 README.md (0 bytes)
/// // └── 📁 src/
/// //     ├── 📁 lib/
/// //     │   └── 📄 mod.rs (0 bytes)
/// //     └── 📄 main.rs (0 bytes)
/// # Ok(())
/// # }
/// ```
///
/// ## Notes
///
/// - Uses traditional tree-drawing characters (├──, └──, │)
/// - File entries include their size in bytes
/// - Directory entries have a trailing slash
/// - Symbolic links are marked with a special icon
/// - Entries are sorted by type first, then alphabetically
pub async fn print_dir_tree<S>(dir: &Dir<S>) -> FsResult<()>
where
    S: IpldStore + Send + Sync,
{
    print_dir_tree_with_depth(dir, vec![]).await
}

/// Internal helper function that implements the recursive tree printing logic.
///
/// This function handles the actual work of printing the tree structure using box-drawing
/// characters. It tracks the prefix for each line to properly draw the tree structure.
#[async_recursion]
async fn print_dir_tree_with_depth<S>(dir: &Dir<S>, mut prefix_parts: Vec<bool>) -> FsResult<()>
where
    S: IpldStore + Send + Sync,
{
    // Collect entries and resolve them first
    let mut entries: Vec<_> = Vec::new();
    for (name, link) in dir.get_entries() {
        let entity = link.resolve_entity(dir.get_store().clone()).await?;
        entries.push((name, link, entity));
    }

    // Sort entries: directories first, then alphabetically within each type
    entries.sort_by(|(name_a, _, entity_a), (name_b, _, entity_b)| {
        match (entity_a, entity_b) {
            // Both are directories - sort by name
            (crate::filesystem::Entity::Dir(_), crate::filesystem::Entity::Dir(_)) => {
                name_a.cmp(name_b)
            }
            // A is directory, B is not - A comes first
            (crate::filesystem::Entity::Dir(_), _) => std::cmp::Ordering::Less,
            // B is directory, A is not - B comes first
            (_, crate::filesystem::Entity::Dir(_)) => std::cmp::Ordering::Greater,
            // Neither is a directory - sort by name
            _ => name_a.cmp(name_b),
        }
    });

    let total = entries.len();

    for (idx, (name, _, entity)) in entries.into_iter().enumerate() {
        let is_last = idx == total - 1;
        let prefix = build_prefix(&prefix_parts);
        let connector = if is_last { LEAF } else { BRANCH };

        match entity {
            crate::filesystem::Entity::Dir(subdir) => {
                println!("{}{} 📁 {}/", prefix, connector, name);
                prefix_parts.push(!is_last);
                print_dir_tree_with_depth(&subdir, prefix_parts.clone()).await?;
                prefix_parts.pop();
            }
            crate::filesystem::Entity::File(file) => {
                let size = file.get_size().await?;
                println!("{}{} 📄 {} ({} bytes)", prefix, connector, name, size);
            }
            _ => println!("{}{} 🔗 {}", prefix, connector, name),
        }
    }

    Ok(())
}

/// Builds the prefix string for a tree line based on the parent levels.
///
/// This helper function creates the proper indentation and vertical lines
/// for each level of the tree.
fn build_prefix(parts: &[bool]) -> String {
    parts
        .iter()
        .map(|&has_sibling| {
            if has_sibling {
                format!("{}{}", VERTICAL, " ".repeat(INDENT_SIZE - 1))
            } else {
                " ".repeat(INDENT_SIZE)
            }
        })
        .collect()
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::MemoryStore;
    use tokio::io::AsyncWriteExt;

    use crate::filesystem::File;

    use super::*;

    /// Note on test execution:
    /// These tests are marked with `#[ignore]` when running with `cargo test` due to
    /// stdout redirection issues in parallel test execution. They work correctly with
    /// `cargo nextest run` which handles process isolation better.
    ///
    /// To run these tests:
    /// - Use `cargo nextest run` (recommended)
    /// - Or use `cargo test -- --ignored --nocapture` to run them sequentially
    ///   (--nocapture is required as the tests need to capture stdout)

    #[ignore = "uses stdout redirection which conflicts with parallel test execution. Use cargo nextest run or cargo test -- --ignored --nocapture"]
    #[tokio::test]
    async fn test_print_dir_tree_empty_directory() -> FsResult<()> {
        let store = MemoryStore::default();
        let dir = Dir::new(store);

        let output = helper::capture_output(|| async { print_dir_tree(&dir).await }).await?;
        assert_eq!(output, ""); // Empty directory should produce no output

        Ok(())
    }

    #[ignore = "uses stdout redirection which conflicts with parallel test execution. Use cargo nextest run or cargo test -- --ignored --nocapture"]
    #[tokio::test]
    async fn test_print_dir_tree_single_file() -> FsResult<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create a file with some content
        let mut file = File::new(store);
        {
            let mut output = file.get_output_stream();
            output.write_all(b"test content").await?;
            output.flush().await?;
            drop(output);
        }
        dir.put_adapted_file("test.txt", file).await?;

        let output = helper::capture_output(|| async { print_dir_tree(&dir).await }).await?;
        assert_eq!(output, "└── 📄 test.txt (12 bytes)\n");

        Ok(())
    }

    #[ignore = "uses stdout redirection which conflicts with parallel test execution. Use cargo nextest run or cargo test -- --ignored --nocapture"]
    #[tokio::test]
    async fn test_print_dir_tree_nested_structure() -> FsResult<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create a nested directory structure with mixed order
        dir.find_or_create("src/main.rs", true).await?;
        dir.find_or_create("docs/README.md", true).await?;
        dir.find_or_create("src/lib/mod.rs", true).await?;

        let output = helper::capture_output(|| async { print_dir_tree(&dir).await }).await?;
        let expected = "\
├── 📁 docs/
│   └── 📄 README.md (0 bytes)
└── 📁 src/
    ├── 📁 lib/
    │   └── 📄 mod.rs (0 bytes)
    └── 📄 main.rs (0 bytes)
";
        assert_eq!(output, expected);

        Ok(())
    }

    #[ignore = "uses stdout redirection which conflicts with parallel test execution. Use cargo nextest run or cargo test -- --ignored --nocapture"]
    #[tokio::test]
    async fn test_print_dir_tree_complex_structure() -> FsResult<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create a complex directory structure with various entity types in mixed order
        dir.find_or_create("project/src/main.rs", true).await?;
        dir.create_sympathlink("project/config", "../config")
            .await?;
        dir.find_or_create("project/docs/api.md", true).await?;
        dir.find_or_create("project/src/lib.rs", true).await?;
        dir.find_or_create("project/docs/examples/basic.rs", true)
            .await?;

        let output = helper::capture_output(|| async { print_dir_tree(&dir).await }).await?;
        let expected = "\
└── 📁 project/
    ├── 📁 docs/
    │   ├── 📁 examples/
    │   │   └── 📄 basic.rs (0 bytes)
    │   └── 📄 api.md (0 bytes)
    ├── 📁 src/
    │   ├── 📄 lib.rs (0 bytes)
    │   └── 📄 main.rs (0 bytes)
    └── 🔗 config
";
        assert_eq!(output, expected);

        Ok(())
    }

    #[ignore = "uses stdout redirection which conflicts with parallel test execution. Use cargo nextest run or cargo test -- --ignored --nocapture"]
    #[tokio::test]
    async fn test_print_dir_tree_alphabetical_sorting() -> FsResult<()> {
        let store = MemoryStore::default();
        let mut dir = Dir::new(store.clone());

        // Create entries in non-alphabetical order
        dir.find_or_create("z_dir", false).await?;
        dir.find_or_create("a_dir", false).await?;
        dir.find_or_create("m_dir", false).await?;
        dir.find_or_create("z_file.txt", true).await?;
        dir.find_or_create("a_file.txt", true).await?;
        dir.create_sympathlink("z_link", "target").await?;
        dir.create_sympathlink("a_link", "target").await?;

        let output = helper::capture_output(|| async { print_dir_tree(&dir).await }).await?;
        let expected = "\
├── 📁 a_dir/
├── 📁 m_dir/
├── 📁 z_dir/
├── 📄 a_file.txt (0 bytes)
├── 🔗 a_link
├── 📄 z_file.txt (0 bytes)
└── 🔗 z_link
";
        assert_eq!(output, expected);

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use std::{future::Future, io::Read, sync::LazyLock};
    use tempfile::NamedTempFile;
    use tokio::sync::Mutex;

    use super::*;

    /// A global mutex to synchronize stdout redirection across tests.
    ///
    /// This mutex ensures that only one test can redirect stdout at a time,
    /// preventing conflicts when running tests in parallel with `cargo test`.
    /// The `gag` crate's stdout redirection is process-wide, so we need this
    /// synchronization to avoid "Redirect already exists" errors.
    static STDOUT_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    /// Helper function to capture stdout during a test
    pub(super) async fn capture_output<F, Fut>(f: F) -> FsResult<String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = FsResult<()>>,
    {
        // Acquire the mutex to ensure only one test redirects stdout at a time
        let _lock = STDOUT_MUTEX.lock().await;

        // Create a temporary file to capture output
        let temp_file = NamedTempFile::new().unwrap();
        let temp_file_clone = temp_file.reopen().unwrap();

        // Redirect stdout to the temp file
        let _guard = gag::Redirect::stdout(temp_file_clone).unwrap();

        // Run the function that produces output
        f().await?;

        // Drop the guard to ensure output is flushed
        drop(_guard);

        // Read the captured output
        let mut output = String::new();
        let mut file = temp_file.reopen().unwrap();
        file.read_to_string(&mut output).unwrap();
        Ok(output)
    }
}

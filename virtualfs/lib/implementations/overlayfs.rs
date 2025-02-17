use std::{
    path::{Path, PathBuf},
    pin::Pin,
};

use async_recursion::async_recursion;
use async_trait::async_trait;
use getset::Getters;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{
    error::VfsError, filesystem::VirtualFileSystem, Metadata, ModeType, PathSegment, VfsResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The prefix for whiteout files
const WHITEOUT_PREFIX: &str = ".wh.";

/// The marker for opaque directories
const OPAQUE_MARKER: &str = ".wh..wh..opq";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A filesystem implementation that combines multiple filesystems into a single logical filesystem,
/// following the OCI (Open Container Initiative) specification for overlay filesystems.
///
/// ## OCI Compatibility
///
/// This implementation adheres to the OCI specification for handling overlay filesystems:
///
/// - Supports OCI whiteout files (`.wh.` prefixed files) to mark deleted files
/// - Handles opaque directory markers (`.wh..wh..opq`) to mask lower layer directories
///
/// ## Layer Structure
///
/// The overlay filesystem consists of:
/// - A single top layer (upperdir) that is writable
/// - Zero or more lower layers that are read-only
///
/// ## Layer Ordering
///
/// When creating an overlay filesystem, layers are provided in order from lowest to highest:
/// ```
/// use std::path::Path;
/// use virtualfs::{MemoryFileSystem, VirtualFileSystem, OverlayFileSystem};
/// use tokio::io::AsyncReadExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create two memory filesystems as layers
/// let mut lower_fs = MemoryFileSystem::new();
/// let mut upper_fs = MemoryFileSystem::new();
///
/// // Add a file to the lower layer
/// lower_fs.create_file(Path::new("lower.txt"), false).await?;
/// lower_fs.write_file(
///     Path::new("lower.txt"),
///     0,
///     Box::pin(std::io::Cursor::new(b"lower content".to_vec()))
/// ).await?;
///
/// // Add a file to the upper layer
/// upper_fs.create_file(Path::new("upper.txt"), false).await?;
/// upper_fs.write_file(
///     Path::new("upper.txt"),
///     0,
///     Box::pin(std::io::Cursor::new(b"upper content".to_vec()))
/// ).await?;
///
/// // Create overlay with lower_fs as the bottom layer and upper_fs as the top layer
/// let overlay = OverlayFileSystem::new(vec![
///     Box::new(lower_fs) as _,    // First layer (bottom)
///     Box::new(upper_fs) as _,    // Last layer becomes the top layer (upperdir)
/// ])?;
///
/// // Both files are visible through the overlay
/// assert!(overlay.exists(Path::new("lower.txt")).await?);
/// assert!(overlay.exists(Path::new("upper.txt")).await?);
///
/// // Read content from lower layer
/// let mut reader = overlay.read_file(Path::new("lower.txt"), 0, u64::MAX).await?;
/// let mut content = String::new();
/// reader.read_to_string(&mut content).await?;
/// assert_eq!(content, "lower content");
///
/// // Create a new file in the overlay (goes to upper layer)
/// overlay.create_file(Path::new("new.txt"), false).await?;
/// overlay.write_file(
///     Path::new("new.txt"),
///     0,
///     Box::pin(std::io::Cursor::new(b"new content".to_vec()))
/// ).await?;
///
/// // The new file exists only in the upper layer
/// assert!(overlay.get_top_layer().exists(Path::new("new.txt")).await?);
///
/// # Ok(())
/// # }
/// ```
///
/// The last layer in the provided sequence becomes the top layer (upperdir), while
/// the others become read-only lower layers. This matches the OCI specification where:
/// - The top layer (upperdir) handles all modifications
/// - Lower layers provide the base content
/// - Changes in the top layer shadow content in lower layers
///
/// ## Layer Behavior
///
/// - All write operations occur in the top layer
/// - When reading, the top layer takes precedence over lower layers
/// - Whiteout files in the top layer hide files from lower layers
/// - Opaque directory markers completely mask lower layer directory contents
#[derive(Getters)]
#[getset(get = "pub with_prefix")]
pub struct OverlayFileSystem {
    /// The writable top layer (upperdir) where all modifications occur
    top_layer: Box<dyn VirtualFileSystem + Send + Sync>,

    /// The read-only lower layers, ordered from bottom to top
    lower_layers: Vec<Box<dyn VirtualFileSystem + Send + Sync>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl OverlayFileSystem {
    /// Creates a new overlay filesystem from a sequence of layers.
    ///
    /// ## Layer Ordering
    ///
    /// Layers must be provided in order from lowest (first) to highest (last):
    /// ```ignore
    /// let overlay = OverlayFileSystem::new(vec![
    ///     lowest_layer,     // First layer (bottom/base)
    ///     middle_layer,     // Middle layer
    ///     highest_layer,    // Last layer (becomes writable top layer)
    /// ])?;
    /// ```
    ///
    /// The last layer in the sequence becomes the writable top layer (upperdir),
    /// while all other layers become read-only lower layers.
    ///
    /// ## Errors
    ///
    /// Returns `OverlayFileSystemRequiresAtLeastOneLayer` if no layers are provided.
    pub fn new(
        layers: impl IntoIterator<Item = Box<dyn VirtualFileSystem + Send + Sync>>,
    ) -> VfsResult<Self> {
        let mut layers = layers.into_iter().collect::<Vec<_>>();
        if layers.is_empty() {
            return Err(VfsError::OverlayFileSystemRequiresAtLeastOneLayer);
        }

        Ok(Self {
            top_layer: layers.pop().unwrap(),
            lower_layers: layers,
        })
    }

    /// Checks if a given path corresponds to a whiteout file.
    ///
    /// Whiteout files are used by overlay filesystems to mark an entry that should be hidden
    /// from lower layers. This function returns `true` if the file name (if available) begins
    /// with the predefined whiteout prefix (`.wh.`).
    fn is_whiteout_file(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with(WHITEOUT_PREFIX))
            .unwrap_or(false)
    }

    /// Recursively ensures that the parent directory of a given path exists in the top (writable) layer.
    ///
    /// This function implements the "copy-up" mechanism: if the parent directory is not present in the top layer
    /// but exists in one of the lower (read-only) layers, it is created in the top layer by copying metadata from
    /// the first lower layer where it is found.
    ///
    /// Additionally, if a whiteout exists in the parent's parent for the specified directory, it will return an error,
    /// as this indicates that the parent directory is masked and should not be available.
    ///
    /// ## Errors
    ///
    /// Returns a `VfsError::ParentDirectoryNotFound` if the parent directory cannot be found in any layer.
    #[async_recursion]
    async fn ensure_parent_in_top(&self, path: &Path) -> VfsResult<()> {
        if let Some(parent) = path.parent() {
            if parent.as_os_str().is_empty() {
                // Root; no parent to ensure.
                return Ok(());
            }

            // Check if parent's parent's whiteout for this parent exists.
            if let Some(pp) = parent.parent() {
                if let Some(pname) = parent.file_name() {
                    let parent_whiteout =
                        pp.join(format!("{}{}", WHITEOUT_PREFIX, pname.to_string_lossy()));
                    if self.get_top_layer().exists(&parent_whiteout).await? {
                        return Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf()));
                    }
                }
            }

            // If already present in the top layer, we are done.
            if self.get_top_layer().exists(parent).await? {
                return Ok(());
            }

            // Check if the parent exists in any lower layer.
            let mut exists_in_lower = false;
            for layer in self.get_lower_layers().iter() {
                if layer.exists(parent).await? {
                    exists_in_lower = true;
                    break;
                }
            }

            if !exists_in_lower {
                return Err(VfsError::ParentDirectoryNotFound(parent.to_path_buf()));
            }

            // Recursively ensure the parent's parent exists in the top layer.
            self.ensure_parent_in_top(parent).await?;

            // Copy-up: create parent's directory in the top layer.
            self.get_top_layer().create_directory(parent).await?;

            // Retrieve parent's metadata from a lower layer and apply it.
            let mut lower_metadata: Option<Metadata> = None;
            for layer in self.get_lower_layers().iter().rev() {
                if layer.exists(parent).await? {
                    lower_metadata = Some(layer.get_metadata(parent).await?);
                    break;
                }
            }

            if let Some(meta) = lower_metadata {
                self.get_top_layer().set_metadata(parent, meta).await?;
            }
        }
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl VirtualFileSystem for OverlayFileSystem {
    async fn exists(&self, path: &Path) -> VfsResult<bool> {
        // If the requested path itself is a whiteout file, return false immediately
        if Self::is_whiteout_file(path) {
            return Ok(false);
        }

        // First check if the path exists in the top layer
        if self.get_top_layer().exists(path).await? {
            return Ok(true);
        }

        // Check if there's a whiteout file in the top layer for this path
        let whiteout_path = if let Some(parent) = path.parent() {
            let file_name = path.file_name().ok_or_else(|| {
                VfsError::InvalidPathComponent("Path must have a file name".to_string())
            })?;
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                file_name.to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        // If a whiteout file exists, the path is considered non-existent
        if self.get_top_layer().exists(&whiteout_path).await? {
            return Ok(false);
        }

        // Check if the closest parent directory is marked as opaque
        // We only need to check the immediate parent since opaque markers only affect their direct children
        if let Some(parent) = path.parent() {
            let opaque_marker = parent.join(OPAQUE_MARKER);
            if self.get_top_layer().exists(&opaque_marker).await? {
                // If parent is opaque and the file doesn't exist in top layer, it doesn't exist
                return Ok(false);
            }
        }

        // Check lower layers in reverse order (top to bottom)
        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn create_file(&self, path: &Path, exists_ok: bool) -> VfsResult<()> {
        // Get the top layer where we'll create the file
        let top_layer = self.get_top_layer();

        // Check if the file exists in any layer
        let file_exists = self.exists(path).await?;
        if file_exists && !exists_ok {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        // Ensure the parent directory exists in the top layer (copy-up if necessary)
        self.ensure_parent_in_top(path).await?;

        // Don't allow creating whiteout files directly
        if Self::is_whiteout_file(path) {
            return Err(VfsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot create whiteout files directly",
            )));
        }

        // If a whiteout file exists for this file, remove it so the file can be created.
        let whiteout_path = if let Some(parent) = path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                path.file_name().unwrap().to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        if self.get_top_layer().exists(&whiteout_path).await? {
            self.get_top_layer().remove(&whiteout_path).await?;
        }

        // Create the file in the top layer
        top_layer.create_file(path, exists_ok).await
    }

    async fn create_directory(&self, path: &Path) -> VfsResult<()> {
        // Check if a directory already exists in the top layer.
        if self.get_top_layer().exists(path).await? {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        // Get the top layer where we'll create the directory
        let top_layer = self.get_top_layer();

        // Ensure the parent directory exists in the top layer (copy-up if necessary)
        self.ensure_parent_in_top(path).await?;

        // Don't allow creating whiteout files directly
        if Self::is_whiteout_file(path) {
            return Err(VfsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot create whiteout files directly",
            )));
        }

        // If a whiteout file exists for this directory, remove it to allow creation.
        let whiteout_path = if let Some(parent) = path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                path.file_name().unwrap().to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        if self.get_top_layer().exists(&whiteout_path).await? {
            self.get_top_layer().remove(&whiteout_path).await?;
        }

        // Create the directory in the top layer
        top_layer.create_directory(path).await
    }

    async fn create_symlink(&self, path: &Path, target: &Path) -> VfsResult<()> {
        // Get the top layer where we'll create the symlink
        let top_layer = self.get_top_layer();

        // Check if the symlink exists in any layer
        let link_exists = self.exists(path).await?;
        if link_exists {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        // Ensure the parent directory exists in the top layer (copy-up if necessary)
        self.ensure_parent_in_top(path).await?;

        // Don't allow creating whiteout files directly
        if Self::is_whiteout_file(path) {
            return Err(VfsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot create whiteout files directly",
            )));
        }

        // Create the symlink in the top layer
        top_layer.create_symlink(path, target).await
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
    ) -> VfsResult<Pin<Box<dyn AsyncRead + Send + Sync + 'static>>> {
        // If path is a whiteout, treat it as not found.
        if Self::is_whiteout_file(path) {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        // If the file exists in the top layer, use it.
        if self.get_top_layer().exists(path).await? {
            return self.get_top_layer().read_file(path, offset, length).await;
        }

        // Check if there's a whiteout for this file in the top layer.
        let whiteout_path = if let Some(parent) = path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                path.file_name().unwrap().to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        if self.get_top_layer().exists(&whiteout_path).await? {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        // If the immediate parent is opaque, lower-layer content is hidden.
        if let Some(parent) = path.parent() {
            let opaque_marker = parent.join(OPAQUE_MARKER);
            if self.get_top_layer().exists(&opaque_marker).await? {
                return Err(VfsError::NotFound(path.to_path_buf()));
            }
        }

        // Otherwise, search lower layers (highest priority first).
        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                return layer.read_file(path, offset, length).await;
            }
        }

        Err(VfsError::NotFound(path.to_path_buf()))
    }

    async fn read_directory(
        &self,
        path: &Path,
    ) -> VfsResult<Box<dyn Iterator<Item = PathSegment> + Send + Sync + 'static>> {
        // Check for a whiteout for this directory.
        let whiteout_path = if let Some(parent) = path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                path.file_name()
                    .ok_or_else(|| VfsError::InvalidPathComponent(
                        "Path must have a file name".to_string()
                    ))?
                    .to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        if self.get_top_layer().exists(&whiteout_path).await? {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        // Check if the parent's opaque marker is set.
        // If so, do not merge lower-layer contents; use top layer exclusively.
        if let Some(parent) = path.parent() {
            let opaque_marker = parent.join(OPAQUE_MARKER);
            if self.get_top_layer().exists(&opaque_marker).await? {
                if self.get_top_layer().exists(path).await? {
                    return self.get_top_layer().read_directory(path).await;
                } else {
                    return Err(VfsError::NotFound(path.to_path_buf()));
                }
            }
        }

        // Build a union of lower and top layer entries.
        use std::collections::HashMap;
        let mut union: HashMap<String, PathSegment> = HashMap::new();

        // Process lower layers in order (lowest first).
        for layer in self.get_lower_layers().iter() {
            if layer.exists(path).await? {
                let entries = layer.read_directory(path).await?;
                for seg in entries.collect::<Vec<_>>() {
                    // We assume that each PathSegment can be converted to a String.
                    let name = seg.to_string();
                    union.insert(name, seg);
                }
            }
        }

        // Process the top layer.
        if self.get_top_layer().exists(path).await? {
            let entries = self.get_top_layer().read_directory(path).await?;
            let top_entries: Vec<PathSegment> = entries.collect();

            // If an opaque marker is present in the top layer directory,
            // then ignore any lower-layer entries.
            if top_entries
                .iter()
                .any(|seg| seg.to_string() == OPAQUE_MARKER)
            {
                union.clear();
            }

            for seg in top_entries {
                let name = seg.to_string();
                if name.starts_with(WHITEOUT_PREFIX) {
                    // A whiteout file hides the corresponding real entry.
                    let real_name = name.trim_start_matches(WHITEOUT_PREFIX).to_string();
                    union.remove(&real_name);
                } else {
                    // Otherwise, the top layer file overrides the lower layers.
                    union.insert(name, seg);
                }
            }
        }

        // Return the merged (union) view as an iterator.
        let result: Vec<PathSegment> = union.into_values().collect();
        Ok(Box::new(result.into_iter()))
    }

    async fn read_symlink(&self, path: &Path) -> VfsResult<PathBuf> {
        if self.get_top_layer().exists(path).await? {
            return self.get_top_layer().read_symlink(path).await;
        }

        let whiteout_path = if let Some(parent) = path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                path.file_name().unwrap().to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        if self.get_top_layer().exists(&whiteout_path).await? {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        if let Some(parent) = path.parent() {
            let opaque_marker = parent.join(OPAQUE_MARKER);
            if self.get_top_layer().exists(&opaque_marker).await? {
                return Err(VfsError::NotFound(path.to_path_buf()));
            }
        }

        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                return layer.read_symlink(path).await;
            }
        }

        Err(VfsError::NotFound(path.to_path_buf()))
    }

    async fn get_metadata(&self, path: &Path) -> VfsResult<Metadata> {
        if self.get_top_layer().exists(path).await? {
            return self.get_top_layer().get_metadata(path).await;
        }

        let whiteout_path = if let Some(parent) = path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                path.file_name().unwrap().to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
        };

        if self.get_top_layer().exists(&whiteout_path).await? {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        if let Some(parent) = path.parent() {
            let opaque_marker = parent.join(OPAQUE_MARKER);
            if self.get_top_layer().exists(&opaque_marker).await? {
                return Err(VfsError::NotFound(path.to_path_buf()));
            }
        }

        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                return layer.get_metadata(path).await;
            }
        }

        Err(VfsError::NotFound(path.to_path_buf()))
    }

    async fn set_metadata(&self, path: &Path, metadata: Metadata) -> VfsResult<()> {
        // Prefer to update metadata in the top layer if the file/directory exists there.
        if self.get_top_layer().exists(path).await? {
            return self.get_top_layer().set_metadata(path, metadata).await;
        }

        // Otherwise, try updating one of the lower layers.
        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                return layer.set_metadata(path, metadata).await;
            }
        }

        Err(VfsError::NotFound(path.to_path_buf()))
    }

    async fn write_file(
        &self,
        path: &Path,
        offset: u64,
        data: Pin<Box<dyn AsyncRead + Send + Sync + 'static>>,
    ) -> VfsResult<()> {
        // Do not allow writes directly on whiteout files.
        if Self::is_whiteout_file(path) {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        let top = self.get_top_layer();

        // If the file exists in the upper (top) layer, simply delegate.
        if top.exists(path).await? {
            return top.write_file(path, offset, data).await;
        }

        // Check if the file exists in any lower layer.
        let mut exists_in_lower = false;
        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                exists_in_lower = true;
                break;
            }
        }

        // Ensure the file's parent exists in the top layer (copy-up if necessary).
        self.ensure_parent_in_top(path).await?;

        if exists_in_lower {
            // Copy-up existing file from a lower layer.
            top.create_file(path, false).await?;
            for layer in self.get_lower_layers().iter().rev() {
                if layer.exists(path).await? {
                    let mut reader = layer.read_file(path, 0, u64::MAX).await?;
                    let mut buffer = Vec::new();

                    reader.read_to_end(&mut buffer).await?;
                    top.write_file(path, 0, Box::pin(std::io::Cursor::new(buffer)))
                        .await?;
                    break;
                }
            }

            // Now delegate the new write operation to the top layer.
            top.write_file(path, offset, data).await
        } else {
            // File does not exist anywhere; create an empty file and then write.
            top.create_file(path, false).await?;
            top.write_file(path, offset, data).await
        }
    }

    async fn remove(&self, path: &Path) -> VfsResult<()> {
        if Self::is_whiteout_file(path) {
            return Err(VfsError::NotFound(path.to_path_buf()));
        }

        let top = self.get_top_layer();

        // If the entity exists in the top layer, delegate removal there.
        if top.exists(path).await? {
            return top.remove(path).await;
        }

        // Otherwise, if it exists only in a lower layer, we must create a whiteout.
        let mut exists_in_lower = false;
        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(path).await? {
                exists_in_lower = true;
                break;
            }
        }

        if exists_in_lower {
            // Ensure the parent's directory is present in the top layer.
            self.ensure_parent_in_top(path).await?;

            let whiteout_path = if let Some(parent) = path.parent() {
                parent.join(format!(
                    "{}{}",
                    WHITEOUT_PREFIX,
                    path.file_name().unwrap().to_string_lossy()
                ))
            } else {
                PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, path.to_string_lossy()))
            };

            if top.exists(&whiteout_path).await? {
                top.remove(&whiteout_path).await?;
            }
            top.create_file(&whiteout_path, false).await
        } else {
            Err(VfsError::NotFound(path.to_path_buf()))
        }
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> VfsResult<()> {
        // Ensure the destination does not already exist.
        if self.exists(new_path).await? {
            return Err(VfsError::AlreadyExists(new_path.to_path_buf()));
        }

        // Ensure the new path's parent exists in the top layer.
        self.ensure_parent_in_top(new_path).await?;

        let top = self.get_top_layer();
        if top.exists(old_path).await? {
            // If the entity is already in the top layer, delegate rename directly.
            return top.rename(old_path, new_path).await;
        }

        // Otherwise, the entity exists only in a lower layer.
        self.ensure_parent_in_top(old_path).await?;
        let mut lower_found = false;
        for layer in self.get_lower_layers().iter().rev() {
            if layer.exists(old_path).await? {
                lower_found = true;
                let metadata = layer.get_metadata(old_path).await?;
                #[cfg(unix)]
                let is_dir = metadata.get_mode().get_type() == Some(ModeType::Directory);
                #[cfg(not(unix))]
                let is_dir = matches!(metadata.get_entity_type(), EntityType::Directory);

                #[cfg(unix)]
                let is_symlink = metadata.get_mode().get_type() == Some(ModeType::Symlink);
                #[cfg(not(unix))]
                let is_symlink = false; // For simplicity on non-Unix platforms

                if is_dir {
                    top.create_directory(old_path).await?;
                } else if is_symlink {
                    let target = layer.read_symlink(old_path).await?;
                    top.create_symlink(old_path, &target).await?;
                } else {
                    top.create_file(old_path, false).await?;
                    let mut reader = layer.read_file(old_path, 0, u64::MAX).await?;
                    let mut buffer = Vec::new();

                    reader.read_to_end(&mut buffer).await?;
                    top.write_file(old_path, 0, Box::pin(std::io::Cursor::new(buffer)))
                        .await?;
                }
                break;
            }
        }

        if !lower_found {
            return Err(VfsError::NotFound(old_path.to_path_buf()));
        }

        // Now that the entity is in the top layer, perform the rename.
        top.rename(old_path, new_path).await?;

        // Create a whiteout for old_path to mask the lower layer.
        let whiteout_path = if let Some(parent) = old_path.parent() {
            parent.join(format!(
                "{}{}",
                WHITEOUT_PREFIX,
                old_path.file_name().unwrap().to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}{}", WHITEOUT_PREFIX, old_path.to_string_lossy()))
        };

        // Mask the lower layer.
        top.create_file(&whiteout_path, false).await
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::ModeType;

    use super::*;

    #[tokio::test]
    async fn test_overlayfs_exists_basic_functionality() {
        // Create test layers
        let lower = helper::create_fs(&["file1.txt", "dir1/file2.txt"]).await;
        let upper = helper::create_fs(&["file3.txt"]).await;

        let overlay = OverlayFileSystem::new(vec![lower, upper]).unwrap();

        // Test files in different layers
        assert!(overlay.exists(Path::new("file1.txt")).await.unwrap()); // Lower layer
        assert!(overlay.exists(Path::new("file3.txt")).await.unwrap()); // Upper layer
        assert!(overlay.exists(Path::new("dir1/file2.txt")).await.unwrap()); // Nested file
        assert!(!overlay.exists(Path::new("nonexistent.txt")).await.unwrap()); // Non-existent
    }

    #[tokio::test]
    async fn test_overlayfs_exists_with_whiteouts() {
        // Create test layers
        let lower = helper::create_fs(&["file1.txt", "file2.txt"]).await;
        let upper = helper::create_fs(&[".wh.file1.txt"]).await; // Whiteout for file1.txt

        let overlay = OverlayFileSystem::new(vec![lower, upper]).unwrap();

        // Test whiteout behavior
        assert!(!overlay.exists(Path::new("file1.txt")).await.unwrap()); // Should be hidden by whiteout
        assert!(overlay.exists(Path::new("file2.txt")).await.unwrap()); // Should still be visible
    }

    #[tokio::test]
    async fn test_overlayfs_exists_with_opaque_directories() {
        // Create test layers
        let lower = helper::create_fs(&["dir1/file1.txt", "dir1/file2.txt"]).await;
        let upper = helper::create_fs(&["dir1/.wh..wh..opq", "dir1/file3.txt"]).await;

        let overlay = OverlayFileSystem::new(vec![lower, upper]).unwrap();

        // Test opaque directory behavior
        assert!(!overlay.exists(Path::new("dir1/file1.txt")).await.unwrap()); // Should be hidden by opaque marker
        assert!(!overlay.exists(Path::new("dir1/file2.txt")).await.unwrap()); // Should be hidden by opaque marker
        assert!(overlay.exists(Path::new("dir1/file3.txt")).await.unwrap()); // Should exist in upper layer
        assert!(overlay.exists(Path::new("dir1")).await.unwrap()); // Directory itself should exist
    }

    #[tokio::test]
    async fn test_overlayfs_exists_layer_precedence() {
        // Create test layers with overlapping files
        let lower = helper::create_fs(&["file1.txt"]).await;
        let middle = helper::create_fs(&["file1.txt", "file2.txt"]).await;
        let upper = helper::create_fs(&["file2.txt"]).await;

        let overlay = OverlayFileSystem::new(vec![lower, middle, upper]).unwrap();

        // Both files should be visible, with upper layers taking precedence
        assert!(overlay.exists(Path::new("file1.txt")).await.unwrap());
        assert!(overlay.exists(Path::new("file2.txt")).await.unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_exists_edge_cases() {
        let lower = helper::create_fs(&["file1.txt"]).await;
        let upper = helper::create_fs(&[".wh.file1.txt", ".wh..wh..opq"]).await;

        let overlay = OverlayFileSystem::new(vec![lower, upper]).unwrap();

        // Test special files and markers
        assert!(!overlay.exists(Path::new(".wh.file1.txt")).await.unwrap()); // Whiteout file itself
        assert!(!overlay.exists(Path::new(".wh..wh..opq")).await.unwrap()); // Opaque marker itself
        assert!(!overlay.exists(Path::new("file1.txt")).await.unwrap()); // Should be hidden by whiteout

        // Test path components that should be rejected
        assert!(overlay.exists(Path::new(".")).await.is_err()); // Current directory
        assert!(overlay.exists(Path::new("..")).await.is_err()); // Parent directory
    }

    // Tests for create_file
    #[tokio::test]
    async fn test_overlayfs_create_file_in_top_layer() {
        // Create test layers with existing content
        let lower = helper::create_fs(&["dir/existing.txt"]).await;
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create a new file - should go to top layer only
        overlay
            .create_file(Path::new("dir/new.txt"), false)
            .await
            .unwrap();

        // Verify file exists in overlay
        assert!(overlay.exists(Path::new("dir/new.txt")).await.unwrap());

        // Verify file exists in top layer but not lower layer
        assert!(overlay
            .get_top_layer()
            .exists(Path::new("dir/new.txt"))
            .await
            .unwrap());

        assert!(!overlay.get_lower_layers()[0]
            .exists(Path::new("dir/new.txt"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_create_file_with_whiteout() {
        // Setup: file exists in lower layer and is whited out in top layer
        let lower = helper::create_fs(&["dir/file.txt"]).await;
        let top = helper::create_fs(&["dir/.wh.file.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Try to create the same file
        let result = overlay.create_file(Path::new("dir/file.txt"), false).await;
        assert!(result.is_ok()); // Should succeed since whiteout means file doesn't exist

        // Verify whiteout file is gone and new file exists in top layer
        assert!(!overlay
            .get_top_layer()
            .exists(Path::new("dir/.wh.file.txt"))
            .await
            .unwrap());

        assert!(overlay
            .get_top_layer()
            .exists(Path::new("dir/file.txt"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_create_file_in_opaque_directory() {
        // Setup: dir with files in lower layer, made opaque in top layer
        let lower = helper::create_fs(&["dir/lower.txt"]).await;
        let top = helper::create_fs(&["dir/.wh..wh..opq"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create new file in opaque directory
        overlay
            .create_file(Path::new("dir/new.txt"), false)
            .await
            .unwrap();

        // Verify file exists in top layer
        assert!(overlay
            .get_top_layer()
            .exists(Path::new("dir/new.txt"))
            .await
            .unwrap());

        // Verify lower layer file is still hidden
        assert!(!overlay.exists(Path::new("dir/lower.txt")).await.unwrap());
    }

    // Tests for create_directory
    #[tokio::test]
    async fn test_overlayfs_create_directory_with_existing_content() {
        // Setup: directory exists in lower layer with content
        let lower = helper::create_fs(&["existing/file.txt"]).await;
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create same directory in overlay
        overlay
            .create_directory(Path::new("existing"))
            .await
            .unwrap();

        // Verify directory exists in top layer
        assert!(overlay
            .get_top_layer()
            .exists(Path::new("existing"))
            .await
            .unwrap());

        // Verify content from lower layer is still visible
        assert!(overlay
            .exists(Path::new("existing/file.txt"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_create_directory_over_whiteout() {
        // Setup: directory is whited out in top layer
        let lower = helper::create_fs(&["dir/file.txt"]).await;
        let top = helper::create_fs(&[".wh.dir"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create the directory
        overlay.create_directory(Path::new("dir")).await.unwrap();

        // Verify whiteout is gone and directory exists in top layer
        assert!(!overlay
            .get_top_layer()
            .exists(Path::new(".wh.dir"))
            .await
            .unwrap());

        assert!(overlay
            .get_top_layer()
            .exists(Path::new("dir"))
            .await
            .unwrap());

        // Verify lower layer content is now visible
        assert!(overlay.exists(Path::new("dir/file.txt")).await.unwrap());
    }

    // Tests for create_symlink
    #[tokio::test]
    async fn test_overlayfs_create_symlink_with_target_in_lower_layer() {
        // Setup: target exists in lower layer
        let lower = helper::create_fs(&["target.txt"]).await;
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create symlink to target
        overlay
            .create_symlink(Path::new("link"), Path::new("target.txt"))
            .await
            .unwrap();

        // Verify symlink exists in top layer only
        assert!(overlay
            .get_top_layer()
            .exists(Path::new("link"))
            .await
            .unwrap());

        assert!(!overlay.get_lower_layers()[0]
            .exists(Path::new("link"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_create_symlink_in_whited_out_directory() {
        // Setup: directory exists in lower layer but is whited out
        let lower = helper::create_fs(&["dir/file.txt"]).await;
        let top = helper::create_fs(&[".wh.dir"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Attempt to create symlink in whited out directory
        let result = overlay
            .create_symlink(Path::new("dir/link"), Path::new("target"))
            .await;

        assert!(matches!(result, Err(VfsError::ParentDirectoryNotFound(_))));
    }

    #[tokio::test]
    async fn test_overlayfs_create_directory_in_opaque_directory() {
        // Setup:
        // Lower layer contains a directory "dir" with a file "file.txt"
        // Top layer contains an opaque marker for "dir"
        let lower = helper::create_fs(&["dir/file.txt"]).await;
        let top = helper::create_fs(&["dir/.wh..wh..opq"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create a new subdirectory "dir/subdir" in the opaque directory
        overlay
            .create_directory(Path::new("dir/subdir"))
            .await
            .unwrap();

        // The opaque marker should prevent lower layer content from being visible;
        // therefore, "dir/file.txt" should remain hidden.
        assert!(!overlay.exists(Path::new("dir/file.txt")).await.unwrap());

        // The new directory "dir/subdir" ought to be visible in the top layer.
        assert!(overlay.exists(Path::new("dir/subdir")).await.unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_create_symlink_in_opaque_directory() {
        // Setup:
        // Lower layer contains a target file "dir/target.txt"
        // Top layer contains an opaque marker for "dir"
        let lower = helper::create_fs(&["dir/target.txt"]).await;
        let top = helper::create_fs(&["dir/.wh..wh..opq"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create a symlink "dir/link" pointing to "target.txt" inside the opaque directory.
        overlay
            .create_symlink(Path::new("dir/link"), Path::new("target.txt"))
            .await
            .unwrap();

        // Verify: The symlink should exist in the top layer.
        assert!(overlay
            .get_top_layer()
            .exists(Path::new("dir/link"))
            .await
            .unwrap());

        // Also, because the opaque marker hides lower layer content,
        // "dir/target.txt" should not be visible in the overlay.
        assert!(!overlay.exists(Path::new("dir/target.txt")).await.unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_create_file_parent_copyup() {
        // Lower layer has "dir/existing.txt"
        let lower = helper::create_fs(&["dir/existing.txt"]).await;

        // Top layer is empty.
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create a new file "dir/new.txt"
        overlay
            .create_file(Path::new("dir/new.txt"), false)
            .await
            .expect("should copy up parent and create file");

        // Verify "dir/new.txt" exists in the top layer.
        assert!(overlay
            .get_top_layer()
            .exists(Path::new("dir/new.txt"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_read_file_upper_layer() {
        // Create top layer containing "file_upper.txt"
        let top = helper::create_fs(&["file_upper.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();

        // Read file from overlay
        let mut reader = overlay
            .read_file(Path::new("file_upper.txt"), 0, 1024)
            .await
            .expect("file in upper layer should be readable");
        let mut buf = Vec::new();
        let n = tokio::io::copy(&mut reader, &mut buf)
            .await
            .expect("read should succeed");

        // Since helper creates empty files, expect empty content.
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn test_overlayfs_read_file_lower_layer() {
        // Lower layer contains "file_lower.txt"
        let lower = helper::create_fs(&["file_lower.txt"]).await;

        // Top layer is empty.
        let top = helper::create_fs(&[]).await;

        // Layers are provided from lowest to highest; the last becomes the writable top.
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let mut reader = overlay
            .read_file(Path::new("file_lower.txt"), 0, 1024)
            .await
            .expect("file in lower layer should be read");
        let mut buf = Vec::new();
        let n = tokio::io::copy(&mut reader, &mut buf)
            .await
            .expect("reading file should succeed");

        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn test_overlayfs_read_file_whiteout() {
        // Lower layer contains "file_whiteout.txt"
        let lower = helper::create_fs(&["file_whiteout.txt"]).await;

        // Top layer has a whiteout file for "file_whiteout.txt"
        let top = helper::create_fs(&[".wh.file_whiteout.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let result = overlay
            .read_file(Path::new("file_whiteout.txt"), 0, 1024)
            .await;

        assert!(matches!(result, Err(VfsError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_overlayfs_read_directory_upper_layer() {
        // Top layer: create "dir/child.txt"
        let top = helper::create_fs(&["dir/child.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();

        let iter = overlay
            .read_directory(Path::new("dir"))
            .await
            .expect("directory should be readable");

        let segments: Vec<String> = iter.map(|s| s.to_string()).collect();

        // Expect a child entry "child.txt" (or "child", depending on how PathSegment is defined)
        assert!(segments.iter().any(|name| name.contains("child")));
    }

    #[tokio::test]
    async fn test_overlayfs_read_directory_lower_layer() {
        // Lower layer: "dir/child_lower.txt"
        let lower = helper::create_fs(&["dir/child_lower.txt"]).await;
        // Top layer is empty.
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let iter = overlay
            .read_directory(Path::new("dir"))
            .await
            .expect("directory in lower layer should be readable");
        let segments: Vec<String> = iter.map(|seg| seg.to_string()).collect();
        assert!(segments.iter().any(|s| s.contains("child_lower")));
    }

    #[tokio::test]
    async fn test_overlayfs_read_directory_whiteout() {
        // Lower layer has "dir/child.txt"
        let lower = helper::create_fs(&["dir/child.txt"]).await;
        // Top layer whiteouts the directory "dir" with ".wh.dir"
        let top = helper::create_fs(&[".wh.dir"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let result = overlay.read_directory(Path::new("dir")).await;
        assert!(matches!(result, Err(VfsError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_overlayfs_read_symlink_upper_layer() {
        // Create an empty top layer and then create a symlink using overlay
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();
        overlay
            .create_symlink(Path::new("link"), Path::new("target_upper.txt"))
            .await
            .expect("symlink creation in top layer should succeed");

        let target = overlay
            .read_symlink(Path::new("link"))
            .await
            .expect("symlink should be readable");
        assert_eq!(target, PathBuf::from("target_upper.txt"));
    }

    #[tokio::test]
    async fn test_overlayfs_read_symlink_lower_layer() {
        // Create a lower layer and add a symlink "link" -> "target_lower.txt".
        // Since helper::create_fs does not distinguish symlinks, we call create_symlink directly on the lower FS.
        let lower = helper::create_fs(&[]).await;
        lower
            .create_symlink(Path::new("link"), Path::new("target_lower.txt"))
            .await
            .expect("symlink creation in lower layer should succeed");
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let target = overlay
            .read_symlink(Path::new("link"))
            .await
            .expect("should read symlink from lower layer");
        assert_eq!(target, PathBuf::from("target_lower.txt"));
    }

    #[tokio::test]
    async fn test_overlayfs_read_symlink_whiteout() {
        // Lower layer creates a symlink "link" -> "target.txt"
        let lower = helper::create_fs(&[]).await;
        lower
            .create_symlink(Path::new("link"), Path::new("target.txt"))
            .await
            .expect("symlink creation should succeed");
        // Top layer whiteouts "link" by providing ".wh.link"
        let top = helper::create_fs(&[".wh.link"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let result = overlay.read_symlink(Path::new("link")).await;
        assert!(matches!(result, Err(VfsError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_overlayfs_get_metadata_upper_layer() {
        // Top layer with "meta.txt"
        let top = helper::create_fs(&["meta.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();

        let metadata = overlay
            .get_metadata(Path::new("meta.txt"))
            .await
            .expect("metadata should be available");
        // Assuming MemoryFileSystem sets size to 0 on empty files.
        assert_eq!(metadata.get_size(), 0);
    }

    #[tokio::test]
    async fn test_overlayfs_get_metadata_lower_layer() {
        // Lower layer with "meta_lower.txt"
        let lower = helper::create_fs(&["meta_lower.txt"]).await;
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let metadata = overlay
            .get_metadata(Path::new("meta_lower.txt"))
            .await
            .expect("metadata from lower layer should be available");
        assert_eq!(metadata.get_size(), 0);
    }

    #[tokio::test]
    async fn test_overlayfs_get_metadata_whiteout() {
        // Lower layer with "meta.txt" but top layer has whiteout ".wh.meta.txt"
        let lower = helper::create_fs(&["meta.txt"]).await;
        let top = helper::create_fs(&[".wh.meta.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        let result = overlay.get_metadata(Path::new("meta.txt")).await;
        assert!(matches!(result, Err(VfsError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_overlayfs_copyup_metadata() {
        // Setup: lower layer contains the directory "dir" via a file inside it.
        let lower = helper::create_fs(&["dir/existing.txt"]).await;

        // Create custom metadata for "dir".
        #[cfg(unix)]
        let mut custom_meta = Metadata::new(ModeType::Directory);
        #[cfg(not(unix))]
        let mut custom_meta = Metadata::new(EntityType::Directory);
        custom_meta.set_size(100);

        // Set the custom metadata on the lower filesystem for "dir".
        lower
            .set_metadata(Path::new("dir"), custom_meta.clone())
            .await
            .unwrap();

        // Create an empty upper (top) layer.
        let top = helper::create_fs(&[]).await;

        // Construct the overlay: lower is the (read-only) lower layer and top is the writable top layer.
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Trigger copy-up by creating a new file inside "dir". This ensures that "dir" is copied up.
        overlay
            .create_file(Path::new("dir/new.txt"), false)
            .await
            .unwrap();

        // Now check that in the top layer, the directory "dir" has the same metadata as in the lower layer.
        let top_dir_meta = overlay
            .get_top_layer()
            .get_metadata(Path::new("dir"))
            .await
            .unwrap();
        assert_eq!(
            top_dir_meta.get_size(),
            custom_meta.get_size(),
            "Metadata size should be copied up correctly"
        );
    }

    #[tokio::test]
    async fn test_overlayfs_write_file_copyup() {
        // Create a lower layer with "test.txt"
        let lower = helper::create_fs(&["test.txt"]).await;
        // Write initial content "Existing" into the lower layer file.
        lower
            .write_file(
                Path::new("test.txt"),
                0,
                Box::pin(std::io::Cursor::new(b"Existing".to_vec())),
            )
            .await
            .unwrap();

        // Prepare an empty upper (top) layer.
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Write new content "NewContent" to "test.txt" via the overlay.
        overlay
            .write_file(
                Path::new("test.txt"),
                0,
                Box::pin(std::io::Cursor::new(b"NewContent".to_vec())),
            )
            .await
            .unwrap();

        // Read from the top layer directly.
        let mut reader = overlay
            .get_top_layer()
            .read_file(Path::new("test.txt"), 0, 1024)
            .await
            .unwrap();
        let mut buf = Vec::new();

        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"NewContent");
    }

    #[tokio::test]
    async fn test_overlayfs_write_file_top_layer() {
        // Create an overlay with a single layer (this becomes the top layer).
        let top = helper::create_fs(&["test_overlayfs_top.txt"]).await;
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();

        // Write new content "Updated" to the file.
        overlay
            .write_file(
                Path::new("test_overlayfs_top.txt"),
                0,
                Box::pin(std::io::Cursor::new(b"Updated".to_vec())),
            )
            .await
            .unwrap();

        // Read and validate the content from the top layer.
        let mut reader = overlay
            .get_top_layer()
            .read_file(Path::new("test_overlayfs_top.txt"), 0, 1024)
            .await
            .unwrap();
        let mut buf = Vec::new();

        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"Updated");
    }

    #[tokio::test]
    async fn test_overlayfs_rename_copyup() {
        // Lower layer has a file "rename.txt" with content "ToRename".
        let lower = helper::create_fs(&["rename.txt"]).await;
        lower
            .write_file(
                Path::new("rename.txt"),
                0,
                Box::pin(std::io::Cursor::new(b"ToRename".to_vec())),
            )
            .await
            .unwrap();

        // Top layer is initially empty.
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Rename the file: this action should copy-up the file and then rename it.
        overlay
            .rename(Path::new("rename.txt"), Path::new("renamed.txt"))
            .await
            .unwrap();

        // Verify that the old path does not exist.
        assert!(!overlay.exists(Path::new("rename.txt")).await.unwrap());
        // Verify that the new file exists in the top layer with the original content.
        let mut reader = overlay
            .get_top_layer()
            .read_file(Path::new("renamed.txt"), 0, 1024)
            .await
            .unwrap();

        let mut buf = Vec::new();

        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"ToRename");
    }

    #[tokio::test]
    async fn test_overlayfs_rename_top_layer() {
        // Prepare a top layer file "topfile.txt" with content "TopContent".
        let top = helper::create_fs(&["topfile.txt"]).await;
        top.write_file(
            Path::new("topfile.txt"),
            0,
            Box::pin(std::io::Cursor::new(b"TopContent".to_vec())),
        )
        .await
        .unwrap();
        // Use a single-layer overlay (the only layer becomes the top layer).
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();

        // Rename "topfile.txt" to "topfile_renamed.txt".
        overlay
            .rename(Path::new("topfile.txt"), Path::new("topfile_renamed.txt"))
            .await
            .unwrap();

        // Verify that the original file no longer exists.
        assert!(!overlay.exists(Path::new("topfile.txt")).await.unwrap());
        // Verify that the new file exists and its content is preserved.
        let mut reader = overlay
            .get_top_layer()
            .read_file(Path::new("topfile_renamed.txt"), 0, 1024)
            .await
            .unwrap();
        let mut buf = Vec::new();

        reader.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"TopContent");
    }

    #[tokio::test]
    async fn test_overlayfs_remove_top_layer() {
        // Create a top layer file "remove_top.txt" with content "DeleteMe".
        let top = helper::create_fs(&["remove_top.txt"]).await;
        top.write_file(
            Path::new("remove_top.txt"),
            0,
            Box::pin(std::io::Cursor::new(b"DeleteMe".to_vec())),
        )
        .await
        .unwrap();
        let overlay = OverlayFileSystem::new(vec![top]).unwrap();

        // Remove the file via the overlay.
        overlay.remove(Path::new("remove_top.txt")).await.unwrap();

        // Verify that the file no longer exists.
        assert!(!overlay.exists(Path::new("remove_top.txt")).await.unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_remove_whiteout() {
        // Create a lower layer with "remove_lower.txt" containing some content.
        let lower = helper::create_fs(&["remove_lower.txt"]).await;
        lower
            .write_file(
                Path::new("remove_lower.txt"),
                0,
                Box::pin(std::io::Cursor::new(b"Content".to_vec())),
            )
            .await
            .unwrap();

        // Prepare an empty top layer.
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Remove the file via the overlay (should create a whiteout since the file exists only in the lower layer).
        overlay.remove(Path::new("remove_lower.txt")).await.unwrap();

        // Verify that overlay.exists returns false for "remove_lower.txt".
        assert!(!overlay.exists(Path::new("remove_lower.txt")).await.unwrap());

        // Verify that a whiteout file exists in the top layer.
        let whiteout_path = Path::new("remove_lower.txt")
            .parent()
            .unwrap()
            .join(format!("{}{}", WHITEOUT_PREFIX, "remove_lower.txt"));
        assert!(overlay
            .get_top_layer()
            .exists(&whiteout_path)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_overlayfs_read_directory_merged_after_copyup() {
        // Create a lower layer that contains the directory "dir" with a file "lower.txt"
        let lower = helper::create_fs(&["dir/lower.txt"]).await;
        // Create an empty top layer
        let top = helper::create_fs(&[]).await;
        let overlay = OverlayFileSystem::new(vec![lower, top]).unwrap();

        // Create a new file "dir/new.txt" via the overlay.
        // This should trigger the copy-up of the "dir" directory; however,
        // the merged (union) view should still include "lower.txt" from the lower layer.
        overlay
            .create_file(Path::new("dir/new.txt"), false)
            .await
            .unwrap();
        overlay
            .write_file(
                Path::new("dir/new.txt"),
                0,
                Box::pin(std::io::Cursor::new(b"new content".to_vec())),
            )
            .await
            .unwrap();

        // Read the directory "dir" and collect the names from the merged view.
        let entry_iter = overlay.read_directory(Path::new("dir")).await.unwrap();
        let mut entries: Vec<String> = entry_iter.map(|s| s.to_string()).collect();
        entries.sort();

        // Expect the directory to contain both "lower.txt" (from the lower layer) and "new.txt" (from top).
        let expected = vec!["lower.txt".to_string(), "new.txt".to_string()];
        assert_eq!(entries, expected);
    }
}

#[cfg(test)]
mod helper {
    use crate::MemoryFileSystem;

    use super::*;

    // Helper function to create a memory filesystem with some initial files
    pub(super) async fn create_fs(files: &[&str]) -> Box<dyn VirtualFileSystem + Send + Sync> {
        let fs = MemoryFileSystem::new();
        for &path in files {
            let path = Path::new(path);

            // Create all parent directories recursively
            if let Some(parent) = path.parent() {
                // Collect all ancestors (excluding the empty root) and reverse to create from root down
                let mut ancestors: Vec<_> = parent
                    .ancestors()
                    .filter(|p| !p.as_os_str().is_empty())
                    .collect();
                ancestors.reverse();

                // Create each directory in the path
                for dir in ancestors {
                    if fs.exists(dir).await.unwrap() {
                        continue;
                    }

                    // Ignore errors since the directory might already exist
                    let _ = fs.create_directory(dir).await;
                }
            }

            // Create the file
            fs.create_file(path, false).await.unwrap();
        }
        Box::new(fs)
    }
}

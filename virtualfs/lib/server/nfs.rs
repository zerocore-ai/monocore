//! NFS server implementation for the virtual filesystem.
//!
//! This module provides an implementation of the NFSv3 protocol for the virtual filesystem.
//! It handles file operations, metadata management, and path-to-fileid mapping required by the NFS protocol.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use intaglio::{Symbol, SymbolTable};
use nfsserve::{
    nfs::{
        fattr3, fileid3, filename3, ftype3, nfspath3, nfsstat3, nfstime3, sattr3, set_atime,
        set_gid3, set_mode3, set_mtime, set_size3, set_uid3, specdata3,
    },
    vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities},
};
use tokio::sync::Mutex;

#[cfg(not(unix))]
use crate::metadata::EntityType;
#[cfg(unix)]
use crate::metadata::{Mode, ModeType};

use crate::VfsError;
use crate::VirtualFileSystem;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// An NFS server implementation for the virtual filesystem.
///
/// This struct implements the NFSv3 protocol by wrapping a virtual filesystem and managing
/// the mapping between NFS file IDs and filesystem paths. It maintains bidirectional mappings
/// between file IDs and paths using a symbol table for efficient string storage.
///
/// # Fields
/// * `root` - The underlying virtual filesystem implementation
/// * `next_fileid` - Counter for generating unique file IDs
/// * `filenames` - Symbol table for storing path components
/// * `fileid_to_path_map` - Maps file IDs to paths (as sequences of symbols)
/// * `path_to_fileid_map` - Maps paths to file IDs
pub struct VirtualFileSystemNFS<F>
where
    F: VirtualFileSystem + Send + Sync,
{
    root: F,
    next_fileid: AtomicU64,
    filenames: Arc<Mutex<SymbolTable>>,
    fileid_to_path_map: Arc<Mutex<HashMap<fileid3, Vec<Symbol>>>>,
    path_to_fileid_map: Arc<Mutex<HashMap<Vec<Symbol>, fileid3>>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<F> VirtualFileSystemNFS<F>
where
    F: VirtualFileSystem + Send + Sync,
{
    /// Creates a new NFS server instance with the given virtual filesystem.
    ///
    /// ## Arguments
    /// * `root` - The virtual filesystem implementation to use
    ///
    /// ## Returns
    /// A new `VirtualFileSystemNFS` instance initialized with empty mappings and the root directory
    /// assigned file ID 0.
    pub fn new(root: F) -> Self {
        Self {
            root,
            next_fileid: AtomicU64::new(1),
            filenames: Arc::new(Mutex::new(SymbolTable::new())),
            fileid_to_path_map: Arc::new(Mutex::new(HashMap::from([(0, vec![])]))),
            path_to_fileid_map: Arc::new(Mutex::new(HashMap::from([(vec![], 0)]))),
        }
    }

    /// Generates the next unique file ID.
    ///
    /// ## Returns
    /// A new unique file ID.
    fn next_fileid(&self) -> fileid3 {
        self.next_fileid.fetch_add(1, Ordering::SeqCst) as fileid3
    }

    /// Converts a file ID to its corresponding filesystem path.
    ///
    /// ## Arguments
    /// * `id` - The file ID to convert
    ///
    /// ## Returns
    /// The path corresponding to the file ID, or an error if the ID is invalid.
    async fn fileid_to_path(&self, id: fileid3) -> Result<String, nfsstat3> {
        let fileid_to_path_map = self.fileid_to_path_map.lock().await;
        let symbols = fileid_to_path_map.get(&id).ok_or(nfsstat3::NFS3ERR_NOENT)?;
        let filenames = self.filenames.lock().await;

        let path = symbols
            .iter()
            .map(|s| filenames.get(*s).ok_or(nfsstat3::NFS3ERR_STALE))
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        Ok(path)
    }

    /// Converts a path string into a sequence of symbols.
    ///
    /// ## Arguments
    /// * `path` - The path to convert
    ///
    /// ## Returns
    /// A vector of symbols representing the path components, or an error if the path is invalid.
    async fn path_to_symbols(&self, path: impl AsRef<str>) -> Result<Vec<Symbol>, nfsstat3> {
        let path = path.as_ref();

        // Split path into segments, filtering out empty segments
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        // Convert segments to symbols
        let mut path_symbols = Vec::with_capacity(segments.len());
        let mut filenames = self.filenames.lock().await;

        for segment in segments {
            let symbol = filenames
                .intern(segment)
                .map_err(|_| nfsstat3::NFS3ERR_INVAL)?;
            path_symbols.push(symbol);
        }
        drop(filenames);

        Ok(path_symbols)
    }

    /// Gets the file ID for a registered path.
    ///
    /// ## Arguments
    /// * `path` - The path to look up
    ///
    /// ## Returns
    /// The file ID if the path is registered, or None if it isn't.
    async fn get_path_registered_str(
        &self,
        path: impl AsRef<str>,
    ) -> Result<Option<fileid3>, nfsstat3> {
        let path_symbols = self.path_to_symbols(path).await?;
        self.get_path_registered(&path_symbols).await
    }

    /// Gets the file ID for a registered path using symbols.
    ///
    /// ## Arguments
    /// * `path_symbols` - The path components as symbols
    ///
    /// ## Returns
    /// The file ID if the path is registered, or None if it isn't.
    async fn get_path_registered(
        &self,
        path_symbols: &[Symbol],
    ) -> Result<Option<fileid3>, nfsstat3> {
        let path_to_fileid_map = self.path_to_fileid_map.lock().await;
        Ok(path_to_fileid_map.get(path_symbols).copied())
    }

    /// Ensures a path is registered and returns its file ID.
    ///
    /// ## Arguments
    /// * `path` - The path to register
    ///
    /// ## Returns
    /// The file ID for the path, creating a new one if necessary.
    async fn ensure_path_registered_str(&self, path: impl AsRef<str>) -> Result<fileid3, nfsstat3> {
        let path_symbols = self.path_to_symbols(path).await?;
        self.ensure_path_registered(&path_symbols).await
    }

    /// Ensures a path is registered using symbols and returns its file ID.
    ///
    /// ## Arguments
    /// * `path_symbols` - The path components as symbols
    ///
    /// ## Returns
    /// The file ID for the path, creating a new one if necessary.
    async fn ensure_path_registered(&self, path_symbols: &[Symbol]) -> Result<fileid3, nfsstat3> {
        // First check if the path is already registered
        if let Some(existing_id) = self.get_path_registered(path_symbols).await? {
            return Ok(existing_id);
        }

        // Create new mapping
        let fileid = self.next_fileid();
        let mut fileid_to_path_map = self.fileid_to_path_map.lock().await;
        let mut path_to_fileid_map = self.path_to_fileid_map.lock().await;

        fileid_to_path_map.insert(fileid, path_symbols.to_vec());
        path_to_fileid_map.insert(path_symbols.to_vec(), fileid);

        Ok(fileid)
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<F> NFSFileSystem for VirtualFileSystemNFS<F>
where
    F: VirtualFileSystem + Send + Sync + 'static,
{
    fn root_dir(&self) -> fileid3 {
        0
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Construct full path
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Check if the path exists in the underlying filesystem
        if !self
            .root
            .exists(std::path::Path::new(&full_path))
            .await
            .map_err(nfsstat3::from)?
        {
            return Err(nfsstat3::NFS3ERR_NOENT);
        }

        // Ensure path is registered and get its fileid
        self.ensure_path_registered_str(&full_path).await
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get metadata from underlying filesystem
        let metadata = self
            .root
            .exists(std::path::Path::new(&path))
            .await
            .map_err(nfsstat3::from)?;

        if !metadata {
            return Err(nfsstat3::NFS3ERR_NOENT);
        }

        let metadata = self
            .root
            .get_metadata(std::path::Path::new(&path))
            .await
            .map_err(nfsstat3::from)?;

        // Convert metadata to NFS attributes
        Ok(fattr3 {
            ftype: match {
                #[cfg(unix)]
                {
                    metadata.get_type()
                }
                #[cfg(not(unix))]
                {
                    Some(metadata.get_entity_type())
                }
            } {
                #[cfg(unix)]
                Some(ModeType::File) => ftype3::NF3REG,
                #[cfg(unix)]
                Some(ModeType::Directory) => ftype3::NF3DIR,
                #[cfg(unix)]
                Some(ModeType::Symlink) => ftype3::NF3LNK,
                #[cfg(not(unix))]
                Some(EntityType::File) => ftype3::NF3REG,
                #[cfg(not(unix))]
                Some(EntityType::Directory) => ftype3::NF3DIR,
                #[cfg(not(unix))]
                Some(EntityType::Symlink) => ftype3::NF3LNK,
                _ => return Err(nfsstat3::NFS3ERR_INVAL),
            },
            #[cfg(unix)]
            mode: u32::from(*metadata.get_mode()) & 0o777,
            #[cfg(not(unix))]
            mode: 0o755, // Default mode for non-Unix systems
            nlink: 1, // We don't support hard links
            #[cfg(unix)]
            uid: metadata.get_uid(),
            #[cfg(not(unix))]
            uid: 0,
            #[cfg(unix)]
            gid: metadata.get_gid(),
            #[cfg(not(unix))]
            gid: 0,
            size: metadata.get_size(),
            used: metadata.get_size(), // Space used is same as size
            rdev: specdata3 {
                specdata1: 0,
                specdata2: 0,
            },
            fsid: 0, // Single filesystem
            fileid: id,
            atime: nfstime3 {
                seconds: metadata.get_accessed_at().timestamp() as u32,
                nseconds: 0,
            },
            mtime: nfstime3 {
                seconds: metadata.get_modified_at().timestamp() as u32,
                nseconds: 0,
            },
            ctime: nfstime3 {
                seconds: metadata.get_created_at().timestamp() as u32,
                nseconds: 0,
            },
        })
    }

    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        // Get path from fileid
        let path = self.fileid_to_path(id).await?;
        let path = std::path::Path::new(&path);

        // Get current metadata
        let metadata = self.root.exists(path).await.map_err(nfsstat3::from)?;

        if !metadata {
            return Err(nfsstat3::NFS3ERR_NOENT);
        }

        let mut metadata = self.root.get_metadata(path).await.map_err(nfsstat3::from)?;

        // Update mode if specified
        #[cfg(unix)]
        if let set_mode3::mode(mode) = setattr.mode {
            let mode = Mode::from(mode & 0o777);
            metadata.set_permissions(mode.get_permissions());
        }

        // Update uid if specified
        #[cfg(unix)]
        if let set_uid3::uid(uid) = setattr.uid {
            metadata.set_uid(uid);
        }

        // Update gid if specified
        #[cfg(unix)]
        if let set_gid3::gid(gid) = setattr.gid {
            metadata.set_gid(gid);
        }

        // Update size if specified
        if let set_size3::size(size) = setattr.size {
            metadata.set_size(size);
        }

        // Update atime if specified
        match setattr.atime {
            set_atime::SET_TO_SERVER_TIME => {
                metadata.set_accessed_at(Utc::now());
            }
            set_atime::SET_TO_CLIENT_TIME(time) => {
                if let Some(dt) = Utc
                    .timestamp_opt(time.seconds as i64, time.nseconds)
                    .earliest()
                {
                    metadata.set_accessed_at(dt);
                }
            }
            set_atime::DONT_CHANGE => {}
        }

        // Update mtime if specified
        match setattr.mtime {
            set_mtime::SET_TO_SERVER_TIME => {
                metadata.set_accessed_at(Utc::now());
            }
            set_mtime::SET_TO_CLIENT_TIME(time) => {
                if let Some(dt) = Utc
                    .timestamp_opt(time.seconds as i64, time.nseconds)
                    .earliest()
                {
                    metadata.set_accessed_at(dt);
                }
            }
            set_mtime::DONT_CHANGE => {}
        }

        // Save updated metadata
        self.root
            .set_metadata(path, metadata.clone())
            .await
            .map_err(nfsstat3::from)?;

        // Return updated attributes
        self.getattr(id).await
    }

    async fn read(
        &self,
        id: fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        // Get path from fileid
        let path = self.fileid_to_path(id).await?;
        let path = std::path::Path::new(&path);

        // Get file size to determine if we're at EOF
        let metadata = self.root.get_metadata(path).await.map_err(nfsstat3::from)?;
        let file_size = metadata.get_size();

        // If offset is beyond file size, return empty with EOF
        if offset >= file_size {
            return Ok((Vec::new(), true));
        }

        // Read the data
        let mut reader = self
            .root
            .read_file(path, offset, count as u64)
            .await
            .map_err(nfsstat3::from)?;

        // Read into buffer
        use tokio::io::AsyncReadExt;
        let mut buffer = Vec::with_capacity(count as usize);
        let bytes_read = reader
            .read_to_end(&mut buffer)
            .await
            .map_err(VfsError::Io)
            .map_err(nfsstat3::from)?;

        // Check if we've reached EOF
        let eof = (offset + bytes_read as u64) >= file_size;

        Ok((buffer, eof))
    }

    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        // Get path from fileid
        let path = self.fileid_to_path(id).await?;
        let path = std::path::Path::new(&path);

        // Get current file size
        let metadata = self.root.get_metadata(path).await.map_err(nfsstat3::from)?;
        let current_size = metadata.get_size();

        // Reject writes that would create holes (sparse files)
        if offset > current_size {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Create a reader for the data
        let reader = Box::pin(std::io::Cursor::new(data.to_vec()));

        // Write the data
        self.root
            .write_file(path, offset, reader)
            .await
            .map_err(nfsstat3::from)?;

        // Return updated attributes
        self.getattr(id).await
    }

    async fn create(
        &self,
        dirid: fileid3,
        filename: &filename3,
        attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Construct full path
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Create the file in the underlying filesystem
        self.root
            .create_file(std::path::Path::new(&full_path), false)
            .await
            .map_err(nfsstat3::from)?;

        // Get the file's metadata
        let mut metadata = self
            .root
            .get_metadata(std::path::Path::new(&full_path))
            .await
            .map_err(nfsstat3::from)?;

        // Update metadata based on provided attributes
        #[cfg(unix)]
        if let set_mode3::mode(mode) = attr.mode {
            let mode = Mode::from(mode & 0o777);
            metadata.set_permissions(mode.get_permissions());
        }

        #[cfg(unix)]
        if let set_uid3::uid(uid) = attr.uid {
            metadata.set_uid(uid);
        }

        #[cfg(unix)]
        if let set_gid3::gid(gid) = attr.gid {
            metadata.set_gid(gid);
        }

        if let set_size3::size(size) = attr.size {
            metadata.set_size(size);
        }

        // Save updated metadata
        self.root
            .set_metadata(std::path::Path::new(&full_path), metadata)
            .await
            .map_err(nfsstat3::from)?;

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered_str(&full_path).await?;

        // Get final attributes
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn create_exclusive(
        &self,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Construct full path
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Check if file already exists - must fail if it does
        if self
            .root
            .exists(std::path::Path::new(&full_path))
            .await
            .map_err(nfsstat3::from)?
        {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }

        // Create the file in the underlying filesystem
        self.root
            .create_file(std::path::Path::new(&full_path), false)
            .await
            .map_err(nfsstat3::from)?;

        // Ensure path is registered and get its fileid
        self.ensure_path_registered_str(&full_path).await
    }

    async fn mkdir(
        &self,
        dirid: fileid3,
        dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        // Convert dirname bytes to string, ensuring valid UTF-8
        let dirname_str = std::str::from_utf8(dirname).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate dirname doesn't contain path separators
        if dirname_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Construct full path
        let full_path = if parent_path.is_empty() {
            dirname_str.to_string()
        } else {
            format!("{}/{}", parent_path, dirname_str)
        };

        // Create the directory in the underlying filesystem
        self.root
            .create_directory(std::path::Path::new(&full_path))
            .await
            .map_err(nfsstat3::from)?;

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered_str(&full_path).await?;

        // Get the directory's attributes
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Construct full path
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Remove the file/directory
        self.root
            .remove(std::path::Path::new(&full_path))
            .await
            .map_err(nfsstat3::from)?;

        Ok(())
    }

    async fn rename(
        &self,
        from_dirid: fileid3,
        from_filename: &filename3,
        to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        // Convert filenames to strings, ensuring valid UTF-8
        let from_filename_str =
            std::str::from_utf8(from_filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;
        let to_filename_str =
            std::str::from_utf8(to_filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filenames don't contain path separators
        if from_filename_str.contains('/') || to_filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get directory paths
        let from_dir_path = self.fileid_to_path(from_dirid).await?;
        let to_dir_path = self.fileid_to_path(to_dirid).await?;

        // Construct full paths
        let from_path = if from_dir_path.is_empty() {
            from_filename_str.to_string()
        } else {
            format!("{}/{}", from_dir_path, from_filename_str)
        };

        let to_path = if to_dir_path.is_empty() {
            to_filename_str.to_string()
        } else {
            format!("{}/{}", to_dir_path, to_filename_str)
        };

        // Perform the rename
        self.root
            .rename(
                std::path::Path::new(&from_path),
                std::path::Path::new(&to_path),
            )
            .await
            .map_err(nfsstat3::from)
    }

    async fn readdir(
        &self,
        dirid: fileid3,
        start_after: fileid3,
        max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        // Get directory path
        let dir_path = self.fileid_to_path(dirid).await?;

        // Read directory entries
        let entries_iter = self
            .root
            .read_directory(std::path::Path::new(&dir_path))
            .await
            .map_err(nfsstat3::from)?;

        let mut entries = Vec::new();
        let mut found_start = start_after == 0;
        let mut has_more = false;

        // Convert entries to NFS format
        for entry_name in entries_iter {
            // Skip entries until we find start_after
            if !found_start {
                let entry_path = if dir_path.is_empty() {
                    entry_name.to_string()
                } else {
                    format!("{}/{}", dir_path, entry_name)
                };

                if let Some(entry_id) = self.get_path_registered_str(&entry_path).await? {
                    if entry_id == start_after {
                        found_start = true;
                    }
                }
                continue;
            }

            // Construct full path for this entry
            let entry_path = if dir_path.is_empty() {
                entry_name.to_string()
            } else {
                format!("{}/{}", dir_path, entry_name)
            };

            // Get or create fileid for this entry
            let fileid = self.ensure_path_registered_str(&entry_path).await?;

            // Get entry attributes
            let attr = self.getattr(fileid).await?;

            // If we've reached max_entries, note that there are more entries and break
            if entries.len() >= max_entries {
                has_more = true;
                break;
            }

            entries.push(DirEntry {
                fileid,
                name: filename3::from(entry_name.as_bytes()),
                attr,
            });
        }

        Ok(ReadDirResult {
            entries,
            end: !has_more,
        })
    }

    async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Read the symlink target
        let target = self
            .root
            .read_symlink(std::path::Path::new(&path))
            .await
            .map_err(nfsstat3::from)?;

        // Convert to NFS path
        Ok(nfspath3::from(target.to_string_lossy().as_bytes()))
    }

    async fn symlink(
        &self,
        dirid: fileid3,
        linkname: &filename3,
        symlink: &nfspath3,
        attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        // Convert linkname bytes to string, ensuring valid UTF-8
        let linkname_str = std::str::from_utf8(linkname).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Convert target path bytes to string
        let target_path = std::str::from_utf8(symlink).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate linkname doesn't contain path separators
        if linkname_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Construct full path
        let full_path = if parent_path.is_empty() {
            linkname_str.to_string()
        } else {
            format!("{}/{}", parent_path, linkname_str)
        };

        // Create the symlink in the underlying filesystem
        self.root
            .create_symlink(
                std::path::Path::new(&full_path),
                std::path::Path::new(target_path),
            )
            .await
            .map_err(nfsstat3::from)?;

        // Get the symlink's metadata
        let mut metadata = self
            .root
            .get_metadata(std::path::Path::new(&full_path))
            .await
            .map_err(nfsstat3::from)?;

        // Set the file type to symlink
        #[cfg(unix)]
        {
            metadata.set_type(ModeType::Symlink);
        }
        #[cfg(not(unix))]
        {
            metadata.set_entity_type(crate::metadata::EntityType::Symlink);
        }

        // Update metadata based on provided attributes
        #[cfg(unix)]
        if let set_mode3::mode(mode) = attr.mode {
            let mode = Mode::from(mode & 0o777);
            metadata.set_permissions(mode.get_permissions());
        }

        #[cfg(unix)]
        if let set_uid3::uid(uid) = attr.uid {
            metadata.set_uid(uid);
        }

        #[cfg(unix)]
        if let set_gid3::gid(gid) = attr.gid {
            metadata.set_gid(gid);
        }

        // Save updated metadata
        self.root
            .set_metadata(std::path::Path::new(&full_path), metadata)
            .await
            .map_err(nfsstat3::from)?;

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered_str(&full_path).await?;

        // Get final attributes
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tokio;

    #[tokio::test]
    async fn test_virtualfilesystemnfs_lookup() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Test lookup in empty root directory
        let result = fs
            .lookup(root_id, &filename3::from(b"nonexistent".to_vec()))
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Create a file and test lookup
        let filename = filename3::from(b"test.txt".to_vec());
        let (file_id, _) = fs
            .create(root_id, &filename, sattr3::default())
            .await
            .unwrap();
        let looked_up_id = fs.lookup(root_id, &filename).await.unwrap();
        assert_eq!(file_id, looked_up_id);

        // Create a directory and test lookup
        let dirname = filename3::from(b"testdir".to_vec());
        let (dir_id, _) = fs.mkdir(root_id, &dirname).await.unwrap();
        let looked_up_dir_id = fs.lookup(root_id, &dirname).await.unwrap();
        assert_eq!(dir_id, looked_up_dir_id);

        // Test lookup with invalid UTF-8
        let result = fs.lookup(root_id, &filename3::from(vec![0xFF])).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test lookup with path separator
        let result = fs
            .lookup(root_id, &filename3::from(b"invalid/path".to_vec()))
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test lookup in non-existent directory
        let result = fs.lookup(999999, &filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_getattr() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Test getattr on root directory
        let root_attrs = fs.getattr(root_id).await.unwrap();
        assert_eq!(root_attrs.fileid, root_id);
        assert!(matches!(root_attrs.ftype, ftype3::NF3DIR));

        // Create a file and test its attributes
        let filename = filename3::from(b"test.txt".to_vec());
        let (file_id, _) = fs
            .create(root_id, &filename, sattr3::default())
            .await
            .unwrap();
        let attrs = fs.getattr(file_id).await.unwrap();
        assert_eq!(attrs.fileid, file_id);
        assert!(matches!(attrs.ftype, ftype3::NF3REG));
        assert_eq!(attrs.size, 0);
        assert_eq!(attrs.nlink, 1);

        // Test getattr on non-existent file
        let result = fs.getattr(999999).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Create a symlink and test its attributes
        let linkname = filename3::from(b"testlink".to_vec());
        let target = nfspath3::from(b"test.txt".to_vec());
        let (link_id, _) = fs
            .symlink(root_id, &linkname, &target, &sattr3::default())
            .await
            .unwrap();

        let attrs = fs.getattr(link_id).await.unwrap();
        assert_eq!(attrs.fileid, link_id);
        assert!(matches!(attrs.ftype, ftype3::NF3LNK));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_setattr() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Create a test file
        let filename = filename3::from(b"test.txt".to_vec());
        let (file_id, _) = fs
            .create(root_id, &filename, sattr3::default())
            .await
            .unwrap();

        // Test setting file size
        let mut setattr = sattr3::default();
        setattr.size = set_size3::size(1000);
        let attrs = fs.setattr(file_id, setattr).await.unwrap();
        assert_eq!(attrs.size, 1000);

        // Test setting timestamps
        let now = Utc::now();
        let mut setattr = sattr3::default();
        setattr.atime = set_atime::SET_TO_CLIENT_TIME(nfstime3 {
            seconds: now.timestamp() as u32,
            nseconds: 0,
        });
        setattr.mtime = set_mtime::SET_TO_CLIENT_TIME(nfstime3 {
            seconds: now.timestamp() as u32,
            nseconds: 0,
        });
        let attrs = fs.setattr(file_id, setattr).await.unwrap();
        assert_eq!(attrs.atime.seconds, now.timestamp() as u32);
        assert_eq!(attrs.mtime.seconds, now.timestamp() as u32);

        // Test setting server time
        let mut setattr = sattr3::default();
        setattr.atime = set_atime::SET_TO_SERVER_TIME;
        setattr.mtime = set_mtime::SET_TO_SERVER_TIME;
        let attrs = fs.setattr(file_id, setattr).await.unwrap();
        // Server time should be recent
        let now = Utc::now().timestamp() as u32;
        assert!(now - attrs.atime.seconds < 2);
        assert!(now - attrs.mtime.seconds < 2);

        // Test setattr on non-existent file
        let result = fs.setattr(999999, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        #[cfg(unix)]
        {
            // Test setting mode (Unix only)
            let mut setattr = sattr3::default();
            setattr.mode = set_mode3::mode(0o644);
            let attrs = fs.setattr(file_id, setattr).await.unwrap();
            assert_eq!(attrs.mode & 0o777, 0o644);

            // Test setting uid/gid (Unix only)
            let mut setattr = sattr3::default();
            setattr.uid = set_uid3::uid(1000);
            setattr.gid = set_gid3::gid(1000);
            let attrs = fs.setattr(file_id, setattr).await.unwrap();
            assert_eq!(attrs.uid, 1000);
            assert_eq!(attrs.gid, 1000);
        }
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_create() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Test basic file creation
        let filename = filename3::from(b"test.txt".to_vec());
        let (file_id, attrs) = fs
            .create(root_id, &filename, sattr3::default())
            .await
            .unwrap();
        assert!(matches!(attrs.ftype, ftype3::NF3REG));
        assert_eq!(attrs.fileid, file_id);
        assert_eq!(attrs.size, 0);

        // Test creating file in non-existent directory
        let result = fs.create(999999, &filename, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test creating file with invalid filename
        let result = fs
            .create(root_id, &filename3::from(vec![0xFF]), sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test creating file with path separator
        let result = fs
            .create(
                root_id,
                &filename3::from(b"invalid/path".to_vec()),
                sattr3::default(),
            )
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test creating file that already exists
        let result = fs.create(root_id, &filename, sattr3::default()).await;
        println!("result: {:?}", result);
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test creating file with custom attributes
        let mut custom_attr = sattr3::default();
        custom_attr.size = set_size3::size(1000);
        let (_, attrs) = fs
            .create(
                root_id,
                &filename3::from(b"custom.txt".to_vec()),
                custom_attr,
            )
            .await
            .unwrap();
        assert_eq!(attrs.size, 1000);
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_create_exclusive() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Test basic exclusive file creation
        let filename = filename3::from(b"exclusive.txt".to_vec());
        let file_id = fs.create_exclusive(root_id, &filename).await.unwrap();
        assert!(file_id > 0);

        // Verify file exists
        assert!(fs.lookup(root_id, &filename).await.is_ok());

        // Test creating same file again (should fail)
        let result = fs.create_exclusive(root_id, &filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test creating in non-existent directory
        let result = fs.create_exclusive(999999, &filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test creating with invalid filename
        let result = fs
            .create_exclusive(root_id, &filename3::from(vec![0xFF]))
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test creating with path separator
        let result = fs
            .create_exclusive(root_id, &filename3::from(b"invalid/path".to_vec()))
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_mkdir() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Test basic directory creation
        let dirname = filename3::from(b"testdir".to_vec());
        let (dir_id, attrs) = fs.mkdir(root_id, &dirname).await.unwrap();
        assert!(matches!(attrs.ftype, ftype3::NF3DIR));
        assert_eq!(attrs.fileid, dir_id);

        // Test creating nested directory
        let nested_name = filename3::from(b"nested".to_vec());
        let (nested_id, nested_attrs) = fs.mkdir(dir_id, &nested_name).await.unwrap();
        assert!(matches!(nested_attrs.ftype, ftype3::NF3DIR));
        assert_eq!(nested_attrs.fileid, nested_id);

        // Test creating directory that already exists
        let result = fs.mkdir(root_id, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test creating in non-existent parent
        let result = fs.mkdir(999999, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test creating with invalid name
        let result = fs.mkdir(root_id, &filename3::from(vec![0xFF])).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test creating with path separator
        let result = fs
            .mkdir(root_id, &filename3::from(b"invalid/path".to_vec()))
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Verify directory structure
        let dir_id2 = fs.lookup(root_id, &dirname).await.unwrap();
        assert_eq!(dir_id, dir_id2);
        let nested_id2 = fs.lookup(dir_id, &nested_name).await.unwrap();
        assert_eq!(nested_id, nested_id2);
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_symlink() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Create a target file first
        let target_name = filename3::from(b"target.txt".to_vec());
        let _ = fs
            .create(root_id, &target_name, sattr3::default())
            .await
            .unwrap();

        // Test basic symlink creation
        let linkname = filename3::from(b"testlink".to_vec());
        let target_path = nfspath3::from(b"target.txt".to_vec());
        let (link_id, attrs) = fs
            .symlink(root_id, &linkname, &target_path, &sattr3::default())
            .await
            .unwrap();
        assert!(matches!(attrs.ftype, ftype3::NF3LNK));
        assert_eq!(attrs.fileid, link_id);

        // Verify symlink target
        let read_target = fs.readlink(link_id).await.unwrap();
        assert_eq!(&*read_target, &*target_path);

        // Test creating symlink that already exists
        let result = fs
            .symlink(root_id, &linkname, &target_path, &sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test creating in non-existent directory
        let result = fs
            .symlink(999999, &linkname, &target_path, &sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test creating with invalid linkname
        let result = fs
            .symlink(
                root_id,
                &filename3::from(vec![0xFF]),
                &target_path,
                &sattr3::default(),
            )
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test creating with path separator in linkname
        let result = fs
            .symlink(
                root_id,
                &filename3::from(b"invalid/path".to_vec()),
                &target_path,
                &sattr3::default(),
            )
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test creating symlink with custom attributes
        let mut custom_attr = sattr3::default();
        custom_attr.mode = set_mode3::mode(0o777);
        let linkname2 = filename3::from(b"testlink2".to_vec());
        let (_, attrs2) = fs
            .symlink(root_id, &linkname2, &target_path, &custom_attr)
            .await
            .unwrap();
        #[cfg(unix)]
        assert_eq!(attrs2.mode & 0o777, 0o777);
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_remove() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Test removing a file
        let filename = filename3::from(b"test.txt".to_vec());
        let _ = fs
            .create(root_id, &filename, sattr3::default())
            .await
            .unwrap();

        // Verify file exists before removal
        assert!(fs.lookup(root_id, &filename).await.is_ok());

        // Remove the file
        fs.remove(root_id, &filename).await.unwrap();

        // Verify file no longer exists
        let result = fs.lookup(root_id, &filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test removing a directory
        let dirname = filename3::from(b"testdir".to_vec());
        let _ = fs.mkdir(root_id, &dirname).await.unwrap();

        // Verify directory exists before removal
        assert!(fs.lookup(root_id, &dirname).await.is_ok());

        // Remove the directory
        fs.remove(root_id, &dirname).await.unwrap();

        // Verify directory no longer exists
        let result = fs.lookup(root_id, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test removing a symlink
        let linkname = filename3::from(b"testlink".to_vec());
        let target_path = nfspath3::from(b"target.txt".to_vec());
        let _ = fs
            .symlink(root_id, &linkname, &target_path, &sattr3::default())
            .await
            .unwrap();

        // Verify symlink exists before removal
        assert!(fs.lookup(root_id, &linkname).await.is_ok());

        // Remove the symlink
        fs.remove(root_id, &linkname).await.unwrap();

        // Verify symlink no longer exists
        let result = fs.lookup(root_id, &linkname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test removing from non-existent directory
        let result = fs.remove(999999, &filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test removing non-existent file
        let nonexistent = filename3::from(b"nonexistent.txt".to_vec());
        let result = fs.remove(root_id, &nonexistent).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test removing with invalid filename
        let result = fs.remove(root_id, &filename3::from(vec![0xFF])).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test removing with path separator in name
        let result = fs
            .remove(root_id, &filename3::from(b"invalid/path".to_vec()))
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test removing a non-empty directory
        let parent_dir = filename3::from(b"parent".to_vec());
        let (parent_id, _) = fs.mkdir(root_id, &parent_dir).await.unwrap();
        let child_file = filename3::from(b"child.txt".to_vec());
        fs.create(parent_id, &child_file, sattr3::default())
            .await
            .unwrap();

        // Attempt to remove the non-empty directory
        let result = fs.remove(root_id, &parent_dir).await;
        println!("result: {:?}", result);
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOTEMPTY)));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_read_write() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Create a test file
        let filename = filename3::from(b"test.txt".to_vec());
        let (file_id, _) = fs
            .create(root_id, &filename, sattr3::default())
            .await
            .unwrap();

        // Test writing data
        let test_data = b"Hello, World!".to_vec();
        let attrs = fs.write(file_id, 0, &test_data).await.unwrap();
        assert_eq!(attrs.size, test_data.len() as u64);

        // Test reading data back
        let (read_data, eof) = fs.read(file_id, 0, test_data.len() as u32).await.unwrap();
        assert_eq!(read_data, test_data);
        assert!(eof);

        // Test partial read
        let (partial_data, eof) = fs.read(file_id, 0, 5).await.unwrap();
        assert_eq!(partial_data, b"Hello");
        assert!(!eof);

        // Test read with offset
        let (offset_data, eof) = fs.read(file_id, 7, 5).await.unwrap();
        assert_eq!(offset_data, b"World");
        assert!(!eof);

        // Test read beyond EOF
        let (empty_data, eof) = fs.read(file_id, 100, 5).await.unwrap();
        assert!(empty_data.is_empty());
        assert!(eof);

        // Test writing with offset
        let append_data = b", Rust!".to_vec();
        let attrs = fs
            .write(file_id, test_data.len() as u64, &append_data)
            .await
            .unwrap();
        assert_eq!(attrs.size, (test_data.len() + append_data.len()) as u64);

        // Verify complete content
        let (full_data, eof) = fs
            .read(file_id, 0, (test_data.len() + append_data.len()) as u32)
            .await
            .unwrap();
        assert_eq!(full_data, b"Hello, World!, Rust!");
        assert!(eof);

        // Test writing with invalid offset (creating holes is not allowed)
        let result = fs.write(file_id, 100, &test_data).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test reading from non-existent file
        let result = fs.read(999999, 0, 5).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_rename() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Create source file
        let src_name = filename3::from(b"source.txt".to_vec());
        let (src_id, _) = fs
            .create(root_id, &src_name, sattr3::default())
            .await
            .unwrap();

        // Write some data to source file
        let test_data = b"Hello, World!".to_vec();
        fs.write(src_id, 0, &test_data).await.unwrap();

        // Create a directory
        let dir_name = filename3::from(b"testdir".to_vec());
        let (dir_id, _) = fs.mkdir(root_id, &dir_name).await.unwrap();

        // Test simple rename in same directory
        let new_name = filename3::from(b"renamed.txt".to_vec());
        fs.rename(root_id, &src_name, root_id, &new_name)
            .await
            .unwrap();

        // Verify old name doesn't exist
        let result = fs.lookup(root_id, &src_name).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Verify new name exists and has same content
        let new_id = fs.lookup(root_id, &new_name).await.unwrap();
        let (content, _) = fs.read(new_id, 0, test_data.len() as u32).await.unwrap();
        assert_eq!(content, test_data);

        // Test rename across directories
        let target_name = filename3::from(b"moved.txt".to_vec());
        fs.rename(root_id, &new_name, dir_id, &target_name)
            .await
            .unwrap();

        // Verify file moved to new directory
        let moved_id = fs.lookup(dir_id, &target_name).await.unwrap();
        let (content, _) = fs.read(moved_id, 0, test_data.len() as u32).await.unwrap();
        assert_eq!(content, test_data);

        // Test rename of non-existent file
        let result = fs
            .rename(
                root_id,
                &filename3::from(b"nonexistent".to_vec()),
                root_id,
                &filename3::from(b"newname".to_vec()),
            )
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test rename with invalid filenames
        let result = fs
            .rename(
                root_id,
                &filename3::from(b"test/invalid".to_vec()),
                root_id,
                &new_name,
            )
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_readdir() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Create some test files and directories
        let files = vec!["file1.txt", "file2.txt", "file3.txt", "aaa.txt", "zzz.txt"];
        let mut file_ids = Vec::new();

        for name in &files {
            let (id, _) = fs
                .create(
                    root_id,
                    &filename3::from(name.as_bytes()),
                    sattr3::default(),
                )
                .await
                .unwrap();
            file_ids.push(id);
        }

        let dirs = vec!["dir1", "dir2", "dir3"];
        let mut dir_ids = Vec::new();

        for name in &dirs {
            let (id, _) = fs
                .mkdir(root_id, &filename3::from(name.as_bytes()))
                .await
                .unwrap();
            dir_ids.push(id);
        }

        // Test reading entire directory
        let ReadDirResult { entries, end } = fs.readdir(root_id, 0, 100).await.unwrap();
        assert!(end); // Should have read everything
        assert_eq!(entries.len(), files.len() + dirs.len());

        // Verify all files and directories are present
        let entry_names: Vec<String> = entries
            .iter()
            .map(|e| String::from_utf8_lossy(&e.name).into_owned())
            .collect();

        for name in files.iter().chain(dirs.iter()) {
            assert!(entry_names.contains(&name.to_string()));
        }

        // Test pagination
        let ReadDirResult {
            entries: first_page,
            end,
        } = fs.readdir(root_id, 0, 3).await.unwrap();
        assert!(!end); // Should have more entries
        assert_eq!(first_page.len(), 3);

        // Get next page starting after last entry of first page
        let ReadDirResult {
            entries: second_page,
            end,
        } = fs
            .readdir(root_id, first_page.last().unwrap().fileid, 3)
            .await
            .unwrap();
        assert!(!end);
        assert_eq!(second_page.len(), 3);

        // Verify no duplicate entries between pages
        let first_page_names: Vec<String> = first_page
            .iter()
            .map(|e| String::from_utf8_lossy(&e.name).into_owned())
            .collect();
        let second_page_names: Vec<String> = second_page
            .iter()
            .map(|e| String::from_utf8_lossy(&e.name).into_owned())
            .collect();
        assert!(first_page_names
            .iter()
            .all(|name| !second_page_names.contains(name)));

        // Test reading from non-existent directory
        let result = fs.readdir(999999, 0, 100).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test reading from a file (should fail)
        let result = fs.readdir(file_ids[0], 0, 100).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOTDIR)));
    }

    #[tokio::test]
    async fn test_virtualfilesystemnfs_readlink() {
        let fs = helper::setup_fs().await;
        let root_id = fs.root_dir();

        // Create a target file
        let target_name = filename3::from(b"target.txt".to_vec());
        let (target_id, _) = fs
            .create(root_id, &target_name, sattr3::default())
            .await
            .unwrap();

        // Create a symlink
        let link_name = filename3::from(b"link".to_vec());
        let (link_id, _) = fs
            .symlink(
                root_id,
                &link_name,
                &nfspath3::from(b"target.txt".to_vec()),
                &sattr3::default(),
            )
            .await
            .unwrap();

        // Test reading the symlink
        let target = fs.readlink(link_id).await.unwrap();
        assert_eq!(&*target, b"target.txt");

        // Test reading from non-existent symlink
        let result = fs.readlink(999999).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test reading from a regular file (should fail)
        let result = fs.readlink(target_id).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test reading from a directory (should fail)
        let (dir_id, _) = fs
            .mkdir(root_id, &filename3::from(b"testdir".to_vec()))
            .await
            .unwrap();
        let result = fs.readlink(dir_id).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));
    }
}

#[cfg(test)]
mod helper {
    use crate::MemoryFileSystem;

    use super::*;

    pub async fn setup_fs() -> VirtualFileSystemNFS<MemoryFileSystem> {
        let memfs = MemoryFileSystem::new();
        VirtualFileSystemNFS::new(memfs)
    }
}

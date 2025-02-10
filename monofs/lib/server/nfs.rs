use std::collections::HashMap;
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use getset::Getters;
use intaglio::{Symbol, SymbolTable};
use monoutils_store::ipld::ipld::Ipld;
use monoutils_store::{IpldStore, IpldStoreSeekable, MemoryStore, Storable};
use nfsserve::nfs::{
    fattr3, fileid3, filename3, ftype3, nfspath3, nfsstat3, nfstime3, sattr3, set_atime, set_gid3,
    set_mode3, set_mtime, set_size3, set_uid3, specdata3,
};
use nfsserve::vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities};
use tokio::sync::Mutex;

use crate::filesystem::{
    Dir, Entity, EntityType, File, Metadata, SymPathLink, UNIX_ATIME_KEY, UNIX_GID_KEY,
    UNIX_MODE_KEY, UNIX_UID_KEY,
};
use crate::store::FlatFsStore;
use crate::FsError;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default file mode (permissions) for newly created files.
/// Equivalent to 644 in octal (rw-r--r--).
pub const DEFAULT_FILE_MODE: u32 = 0o644;

/// Default directory mode (permissions) for newly created directories.
/// Equivalent to 755 in octal (rwxr-xr-x).
pub const DEFAULT_DIR_MODE: u32 = 0o755;

/// Default symlink mode (permissions) for newly created symbolic links.
/// Equivalent to 777 in octal (rwxrwxrwx).
pub const DEFAULT_SYMLINK_MODE: u32 = 0o777;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A MonofsNFS that uses an in-memory store for testing and development.
/// This type is not suitable for production use as all data is lost when the process exits.
pub type MemoryMonofsNFS = MonofsNFS<MemoryStore>;

/// A MonofsNFS that uses a flat filesystem store for persistent storage.
/// This is the recommended type for production use.
pub type DiskMonofsNFS = MonofsNFS<FlatFsStore>;

/// An implementation of the NFSv3 server interface backed by a content-addressed store.
///
/// MonofsNFS provides an NFSv3 server implementation that stores all file system
/// data in a content-addressed store. This allows for:
///
/// - Immutable file system history
/// - Deduplication of file content
/// - Efficient file system snapshots
/// - Distributed synchronization
///
/// The server maintains mappings between NFS file IDs and internal paths, and handles
/// all the standard NFS operations like:
///
/// - File/directory creation and removal
/// - Reading and writing files
/// - Directory listing
/// - File attribute management
/// - Symbolic link operations
///
/// ## Limitations
///
/// - Sparse files are not supported. Attempts to write data beyond the current file size
///   will result in an INVAL error. All writes must be contiguous with existing data or
///   start at offset 0 for new files.
///
/// ## Examples
///
/// ```no_run
/// use monofs::server::{MemoryMonofsNFS, MonofsNFS};
/// use monoutils_store::MemoryStore;
///
/// // Create an in-memory server for testing
/// let memory_server = MemoryMonofsNFS::new(MemoryStore::default());
///
/// // Or create a custom server with your own store implementation
/// // let custom_server = MonofsNFS::new(CustomStore::default());
/// ```
#[derive(Debug, Getters)]
pub struct MonofsNFS<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    root: Arc<Mutex<Dir<S>>>,
    next_fileid: AtomicU64,
    filenames: Arc<Mutex<SymbolTable>>,
    fileid_to_path_map: Arc<Mutex<HashMap<fileid3, Vec<Symbol>>>>,
    path_to_fileid_map: Arc<Mutex<HashMap<Vec<Symbol>, fileid3>>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> MonofsNFS<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    /// Creates a new MonofsNFS instance with the given store.
    ///
    /// ## Example
    /// ```rust
    /// use monofs::server::MemoryMonofsNFS;
    /// use monoutils_store::MemoryStore;
    ///
    /// let server = MemoryMonofsNFS::new(MemoryStore::default());
    /// ```
    pub fn new(store: S) -> Self {
        Self {
            root: Arc::new(Mutex::new(Dir::new(store))),
            filenames: Arc::new(Mutex::new(SymbolTable::new())),
            next_fileid: AtomicU64::new(1),
            fileid_to_path_map: Arc::new(Mutex::new(HashMap::from([(0, vec![])]))),
            path_to_fileid_map: Arc::new(Mutex::new(HashMap::from([(vec![], 0)]))),
        }
    }

    fn next_fileid(&self) -> fileid3 {
        self.next_fileid.fetch_add(1, Ordering::SeqCst)
    }

    /// Converts a file ID to its corresponding path by looking up the symbols in the mapping
    /// and converting them back to strings.
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

    /// Converts a path string into a vector of symbols.
    /// Each path component is converted to a symbol using the server's symbol table.
    /// Empty path components are filtered out.
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

    /// Gets the fileid for a registered path if it exists.
    /// This is a convenience wrapper around `get_path_registered` that takes a string path.
    async fn get_path_registered_str(
        &self,
        path: impl AsRef<str>,
    ) -> Result<Option<fileid3>, nfsstat3> {
        let path_symbols = self.path_to_symbols(path).await?;
        self.get_path_registered(&path_symbols).await
    }

    /// Gets the fileid for a registered path if it exists.
    /// This is the core path lookup function that works directly with symbols.
    async fn get_path_registered(
        &self,
        path_symbols: &[Symbol],
    ) -> Result<Option<fileid3>, nfsstat3> {
        let path_to_fileid_map = self.path_to_fileid_map.lock().await;
        Ok(path_to_fileid_map.get(path_symbols).copied())
    }

    /// Ensures a path is registered in the path-fileid mapping system and returns its fileid.
    /// This is a convenience wrapper around `ensure_path_registered` that takes a string path.
    async fn ensure_path_registered_str(&self, path: impl AsRef<str>) -> Result<fileid3, nfsstat3> {
        let path_symbols = self.path_to_symbols(path).await?;
        self.ensure_path_registered(&path_symbols).await
    }

    /// Ensures a path is registered in the path-fileid mapping system and returns its fileid.
    /// This is the core path registration function that works directly with symbols.
    /// If the path is already registered, returns the existing fileid.
    /// If not, creates a new fileid and registers the bidirectional mappings.
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

    /// Helper method to update attributes on an entity's metadata
    async fn update_attributes(metadata: &mut Metadata<S>, attr: &sattr3) -> Result<(), nfsstat3> {
        // Update mode
        match attr.mode {
            set_mode3::Void => {}
            set_mode3::mode(mode) => {
                metadata
                    .set_attribute(UNIX_MODE_KEY, mode)
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update uid
        match attr.uid {
            set_uid3::Void => {}
            set_uid3::uid(uid) => {
                metadata
                    .set_attribute(UNIX_UID_KEY, uid)
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update gid
        match attr.gid {
            set_gid3::Void => {}
            set_gid3::gid(gid) => {
                metadata
                    .set_attribute(UNIX_GID_KEY, gid)
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update atime
        match attr.atime {
            set_atime::DONT_CHANGE => {}
            set_atime::SET_TO_SERVER_TIME => {
                let now = Utc::now();
                metadata
                    .set_attribute(UNIX_ATIME_KEY, now.timestamp())
                    .await
                    .map_err(nfsstat3::from)?;
            }
            set_atime::SET_TO_CLIENT_TIME(atime) => {
                metadata
                    .set_attribute(UNIX_ATIME_KEY, atime.seconds)
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update mtime
        match attr.mtime {
            set_mtime::DONT_CHANGE => {}
            set_mtime::SET_TO_SERVER_TIME => {
                let now = Utc::now();
                metadata.set_modified_at(now);
            }
            set_mtime::SET_TO_CLIENT_TIME(mtime) => {
                // Combine `seconds` and `nseconds` properly
                metadata.set_modified_at(
                    Utc.timestamp_opt(mtime.seconds as i64, mtime.nseconds)
                        .unwrap(),
                );
            }
        }

        Ok(())
    }

    /// Constructs NFS attributes (fattr3) from metadata.
    async fn construct_attributes(
        metadata: &Metadata<S>,
        size: u64,
        id: fileid3,
    ) -> Result<fattr3, nfsstat3> {
        Ok(fattr3 {
            ftype: match metadata.get_entity_type() {
                EntityType::File => ftype3::NF3REG,
                EntityType::Dir => ftype3::NF3DIR,
                EntityType::SymCidLink | EntityType::SymPathLink => ftype3::NF3LNK,
            },
            // Default mode is 0o755 (rwxr-xr-x) if not set or invalid
            mode: metadata
                .get_attribute(UNIX_MODE_KEY)
                .await
                .map_err(nfsstat3::from)?
                .and_then(|ipld| match &*ipld {
                    Ipld::String(s) => s.parse().ok(),
                    Ipld::Integer(i) => {
                        if *i >= 0 {
                            Some(i.abs() as u32)
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .unwrap_or(match metadata.get_entity_type() {
                    EntityType::File => DEFAULT_FILE_MODE,
                    EntityType::Dir => DEFAULT_DIR_MODE,
                    EntityType::SymCidLink | EntityType::SymPathLink => DEFAULT_SYMLINK_MODE,
                }),
            nlink: 1, // We don't support hard links
            // Default uid is 0 (root) if not set or invalid
            uid: metadata
                .get_attribute(UNIX_UID_KEY)
                .await
                .map_err(nfsstat3::from)?
                .and_then(|ipld| match &*ipld {
                    Ipld::String(s) => s.parse().ok(),
                    Ipld::Integer(i) => {
                        if *i >= 0 {
                            Some(i.abs() as u32)
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .unwrap_or(507), // TODO: Maybe use the uid of the user that made the request
            // Default gid is 0 (root) if not set or invalid
            gid: metadata
                .get_attribute(UNIX_GID_KEY)
                .await
                .map_err(nfsstat3::from)?
                .and_then(|ipld| match &*ipld {
                    Ipld::String(s) => s.parse().ok(),
                    Ipld::Integer(i) => {
                        if *i >= 0 {
                            Some(i.abs() as u32)
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .unwrap_or(507), // TODO: Maybe use the gid of the user that made the request
            size,
            used: 0, // TODO: Space used is not tracked
            rdev: specdata3 {
                specdata1: 0,
                specdata2: 0,
            },
            fsid: 0,    // Single filesystem
            fileid: id, // Use the provided fileid
            atime: nfstime3 {
                seconds: metadata.get_created_at().timestamp() as u32,
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
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<S> NFSFileSystem for MonofsNFS<S>
where
    S: IpldStoreSeekable + Send + Sync,
{
    fn root_dir(&self) -> fileid3 {
        0
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        tracing::trace!("lookup: dirid: {}, filename: {}", dirid, filename);

        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let root = self.root.lock().await;

        tracing::trace!("parent_path: {}", parent_path);

        // Get parent directory - handle root directory case specially
        let parent_dir = if parent_path.is_empty() {
            &*root
        } else {
            match root.find(&parent_path).await? {
                Some(Entity::Dir(dir)) => dir,
                Some(_) => return Err(nfsstat3::NFS3ERR_NOTDIR),
                None => return Err(nfsstat3::NFS3ERR_NOENT),
            }
        };

        // Check if the entry exists
        if !parent_dir.has_entity(filename_str).await? {
            return Err(nfsstat3::NFS3ERR_NOENT);
        }

        drop(root);

        // Construct full path
        let full_path = join_path(&parent_path, filename_str);

        // Ensure path is registered and get its fileid
        self.ensure_path_registered_str(&full_path).await
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        tracing::trace!("getattr: id: {}", id);

        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get root directory
        let root = self.root.lock().await;

        // Get metadata
        let (metadata, size) = if path.is_empty() {
            (root.get_metadata(), 0)
        } else {
            let entity = root.find(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?;
            (entity.get_metadata(), entity.get_size().await?)
        };

        // Convert to NFS attributes
        Self::construct_attributes(metadata, size, id).await
    }

    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        tracing::trace!("setattr: id: {}, setattr: {:?}", id, setattr);

        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Get metadata
        let (metadata, size) = if path.is_empty() {
            (root.get_metadata_mut(), 0)
        } else {
            let entity = root.find_mut(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?;
            let size = entity.get_size().await?;
            (entity.get_metadata_mut(), size)
        };

        // Update all attributes
        Self::update_attributes(metadata, &setattr).await?;

        // Construct and return updated attributes directly
        Self::construct_attributes(metadata, size, id).await
    }

    async fn read(
        &self,
        id: fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        tracing::trace!("read: id: {}, offset: {}, count: {}", id, offset, count);

        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get root directory
        let root = self.root.lock().await;

        // Get the file
        let entity = if path.is_empty() {
            return Err(nfsstat3::NFS3ERR_INVAL); // Root cannot be read
        } else {
            root.find(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?
        };

        // Ensure it's a file and read its content
        match entity {
            Entity::File(file) => {
                use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

                if offset >= file.get_size().await? {
                    return Ok((Vec::new(), true));
                }

                let mut input_stream = file.get_input_stream().await.map_err(|e| {
                    tracing::error!("Failed to get input stream: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                // Seek to offset
                input_stream
                    .seek(SeekFrom::Start(offset))
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to seek: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;

                // Read requested bytes
                let mut buffer = vec![0; count as usize];
                let bytes_read = input_stream.read(&mut buffer).await.map_err(|e| {
                    tracing::error!("Failed to read: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                // Truncate buffer to actual bytes read
                buffer.truncate(bytes_read);

                // Check if we've reached the end by trying to read one more byte
                let mut peek_buf = [0u8; 1];
                let reached_end = input_stream.read(&mut peek_buf).await.map_err(|e| {
                    tracing::error!("Failed to peek: {}", e);
                    nfsstat3::NFS3ERR_IO
                })? == 0;

                Ok((buffer, reached_end))
            }
            _ => Err(nfsstat3::NFS3ERR_NOTDIR),
        }
    }

    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        tracing::trace!("write: id: {}, offset: {}, data: {:?}", id, offset, data);

        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Get the file
        let entity = if path.is_empty() {
            return Err(nfsstat3::NFS3ERR_INVAL); // Root cannot be written
        } else {
            root.find_mut(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?
        };

        // Ensure it's a file and write its content
        match entity {
            Entity::File(file) => {
                use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};

                // Get original file size
                let original_size = file.get_size().await.map_err(|e| {
                    tracing::error!("Failed to get original file size: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                // Reject writes that would create holes (sparse files)
                if offset > original_size {
                    tracing::error!("Attempted to write at offset {} beyond file size {}, which would create a sparse file", offset, original_size);
                    return Err(nfsstat3::NFS3ERR_INVAL);
                }

                // First checkpoint the file to create a versioned copy
                let checkpoint_cid = file.checkpoint().await.map_err(|e| {
                    tracing::error!("Failed to checkpoint file: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                // Load the checkpointed version as our original file
                let original_file = File::load(&checkpoint_cid, file.get_store().clone())
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to load checkpointed file: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;

                // Create output stream for the new version
                let mut output = file.get_output_stream();

                // If we're not writing at the start, copy existing data up to offset
                if offset > 0 {
                    let mut input = original_file.get_input_stream().await.map_err(|e| {
                        tracing::error!("Failed to get input stream: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;

                    // Copy data up to offset
                    let mut buffer = vec![0u8; offset as usize];
                    let bytes_read = input.read(&mut buffer).await.map_err(|e| {
                        tracing::error!("Failed to read existing data: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;
                    buffer.truncate(bytes_read);

                    output.write_all(&buffer).await.map_err(|e| {
                        tracing::error!("Failed to write existing data: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;
                }

                // Write the new data
                output.write_all(data).await.map_err(|e| {
                    tracing::error!("Failed to write new data: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                // If there's existing data after our write, append it
                let end_offset = offset + data.len() as u64;
                if end_offset < original_size {
                    let mut input = original_file.get_input_stream().await.map_err(|e| {
                        tracing::error!("Failed to get input stream: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;

                    // Seek to where we ended our write
                    input.seek(SeekFrom::Start(end_offset)).await.map_err(|e| {
                        tracing::error!("Failed to seek input stream: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;

                    // Read and write the remaining data
                    let mut buffer = vec![0u8; (original_size - end_offset) as usize];
                    let bytes_read = input.read(&mut buffer).await.map_err(|e| {
                        tracing::error!("Failed to read remaining data: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;
                    buffer.truncate(bytes_read);

                    output.write_all(&buffer).await.map_err(|e| {
                        tracing::error!("Failed to write remaining data: {}", e);
                        nfsstat3::NFS3ERR_IO
                    })?;
                }

                // Finalize the write
                output.flush().await.map_err(|e| {
                    tracing::error!("Failed to finalize write: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                drop(output);

                // Get updated attributes
                let final_size = file.get_size().await.map_err(|e| {
                    tracing::error!("Failed to get final file size: {}", e);
                    nfsstat3::NFS3ERR_IO
                })?;

                Self::construct_attributes(file.get_metadata(), final_size, id).await
            }
            _ => Err(nfsstat3::NFS3ERR_NOTDIR),
        }
    }

    async fn create(
        &self,
        dirid: fileid3,
        filename: &filename3,
        attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        tracing::trace!(
            "create: dirid: {}, filename: {:?}, attr: {:?}",
            dirid,
            filename,
            attr
        );
        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Get parent directory - handle root directory case specially
        let parent_dir = if parent_path.is_empty() {
            &mut *root
        } else {
            match root.find_mut(&parent_path).await? {
                Some(Entity::Dir(dir)) => dir,
                Some(_) => return Err(nfsstat3::NFS3ERR_NOTDIR),
                None => return Err(nfsstat3::NFS3ERR_NOENT),
            }
        };

        // Check if file already exists
        if parent_dir.has_entry(filename_str)? {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }

        // Create new file
        let entity = parent_dir.find_or_create(filename_str, true).await?;

        // Apply attributes if provided
        if let Entity::File(ref mut file) = entity {
            // Set default mode if not specified
            if matches!(attr.mode, set_mode3::Void) {
                file.get_metadata_mut()
                    .set_attribute(UNIX_MODE_KEY, DEFAULT_FILE_MODE.to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }

            // Update all attributes
            Self::update_attributes(file.get_metadata_mut(), &attr).await?;

            // Handle size separately since it requires truncating the file
            if let set_size3::size(_) = attr.size {
                file.truncate();
            }
        } else {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        drop(root);

        // Construct full path and ensure it is registered
        let full_path = join_path(&parent_path, filename_str);

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered_str(&full_path).await?;

        // Get the attributes of the created file
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn create_exclusive(
        &self,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        tracing::trace!(
            "create_exclusive: dirid: {}, filename: {:?}",
            dirid,
            filename
        );
        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Get parent directory - handle root directory case specially
        let parent_dir = if parent_path.is_empty() {
            &mut *root
        } else {
            match root.find_mut(&parent_path).await? {
                Some(Entity::Dir(dir)) => dir,
                Some(_) => return Err(nfsstat3::NFS3ERR_NOTDIR),
                None => return Err(nfsstat3::NFS3ERR_NOENT),
            }
        };

        // Check if file already exists - for exclusive create, this must fail
        if parent_dir.has_entry(filename_str)? {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }

        // Create new file with default attributes
        let entity = parent_dir.find_or_create(filename_str, true).await?;

        // Apply default attributes
        if let Entity::File(ref mut file) = entity {
            // Set default mode
            file.get_metadata_mut()
                .set_attribute(UNIX_MODE_KEY, DEFAULT_FILE_MODE.to_string())
                .await
                .map_err(nfsstat3::from)?;
        } else {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        drop(root);

        // Construct full path and ensure it is registered
        let full_path = join_path(&parent_path, filename_str);

        // Ensure path is registered and get its fileid
        self.ensure_path_registered_str(&full_path).await
    }

    async fn mkdir(
        &self,
        dirid: fileid3,
        dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        tracing::trace!("mkdir: dirid: {}, dirname: {:?}", dirid, dirname);
        // Convert dirname bytes to string, ensuring valid UTF-8
        let dirname_str = str::from_utf8(dirname).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate dirname doesn't contain path separators
        if dirname_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Get parent directory - handle root directory case specially
        let parent_dir = if parent_path.is_empty() {
            &mut *root
        } else {
            match root.find_mut(&parent_path).await? {
                Some(Entity::Dir(dir)) => dir,
                Some(_) => return Err(nfsstat3::NFS3ERR_NOTDIR),
                None => return Err(nfsstat3::NFS3ERR_NOENT),
            }
        };

        // Check if directory already exists
        if parent_dir.has_entry(dirname_str)? {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }

        // Create new directory
        let entity = parent_dir.find_or_create(dirname_str, false).await?;

        // Apply default attributes
        if let Entity::Dir(ref mut dir) = entity {
            // Set default mode
            dir.get_metadata_mut()
                .set_attribute(UNIX_MODE_KEY, DEFAULT_DIR_MODE.to_string())
                .await
                .map_err(nfsstat3::from)?;
        } else {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        drop(root);

        // Construct full path and ensure it is registered
        let full_path = join_path(&parent_path, dirname_str);

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered_str(&full_path).await?;

        // Get the attributes of the created directory
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        tracing::trace!("remove: dirid: {}, filename: {:?}", dirid, filename);

        // Convert filename bytes to string, ensuring valid UTF-8
        let filename_str = str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filename doesn't contain path separators
        if filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Construct the full path
        let full_path = join_path(&parent_path, filename_str);

        // Use Dir's remove operation
        root.remove(&full_path).await.map_err(nfsstat3::from)
    }

    async fn rename(
        &self,
        from_dirid: fileid3,
        from_filename: &filename3,
        to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        tracing::trace!(
            "rename: from_dirid: {}, from_filename: {:?}, to_dirid: {}, to_filename: {:?}",
            from_dirid,
            from_filename,
            to_dirid,
            to_filename
        );

        // Convert filenames to strings, ensuring valid UTF-8
        let from_filename_str =
            str::from_utf8(from_filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;
        let to_filename_str = str::from_utf8(to_filename).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate filenames don't contain path separators
        if from_filename_str.contains('/') || to_filename_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get directory paths
        let from_dir_path = self.fileid_to_path(from_dirid).await?;
        let to_dir_path = self.fileid_to_path(to_dirid).await?;

        // Construct full paths
        let from_path = join_path(&from_dir_path, from_filename_str);
        let to_path = join_path(&to_dir_path, to_filename_str);

        // Get root directory and use Dir's rename operation
        let mut root = self.root.lock().await;
        root.rename(&from_path, &to_path)
            .await
            .map_err(nfsstat3::from)
    }

    async fn readdir(
        &self,
        dirid: fileid3,
        start_after: fileid3,
        max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        tracing::trace!(
            "readdir: dirid: {}, start_after: {}, max_entries: {}",
            dirid,
            start_after,
            max_entries
        );

        // Get path from fileid
        let dir_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let root = self.root.lock().await;

        // Get directory
        let dir = if dir_path.is_empty() {
            &*root
        } else {
            match root.find(&dir_path).await? {
                Some(Entity::Dir(dir)) => dir,
                Some(_) => return Err(nfsstat3::NFS3ERR_NOTDIR),
                None => return Err(nfsstat3::NFS3ERR_NOENT),
            }
        };

        let mut entries = Vec::new();
        let mut found_start = start_after == 0;
        let mut has_more = false;

        for (name, link) in dir.get_entries() {
            // Skip entries until we find the start_after fileid
            if !found_start {
                let entry_path = join_path(&dir_path, name.as_str());

                // Try to get existing fileid without creating a new one
                if let Some(entry_id) = self.get_path_registered_str(&entry_path).await? {
                    if entry_id == start_after {
                        found_start = true;
                    }
                }
                continue;
            }

            // Resolve the entity to get its metadata
            let entity = link.resolve_entity(dir.get_store().clone()).await?;

            // Get the full path for this entry
            let entry_path = join_path(&dir_path, name.as_str());

            // Get or create fileid for this entry
            let fileid = self.ensure_path_registered_str(&entry_path).await?;

            // Construct attributes for this entry
            let attr =
                Self::construct_attributes(entity.get_metadata(), entity.get_size().await?, fileid)
                    .await?;

            // If we've reached max_entries, note that there are more entries and break
            if entries.len() >= max_entries {
                has_more = true;
                break;
            }

            entries.push(DirEntry {
                fileid,
                name: filename3::from(name.as_str().as_bytes()),
                attr,
            });
        }

        Ok(ReadDirResult {
            entries,
            end: !has_more, // Set end to true only if we've processed all entries
        })
    }

    async fn symlink(
        &self,
        dirid: fileid3,
        linkname: &filename3,
        symlink: &nfspath3,
        attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        tracing::trace!(
            "symlink: dirid: {}, linkname: {:?}, symlink: {:?}, attr: {:?}",
            dirid,
            linkname,
            symlink,
            attr
        );

        // Convert linkname bytes to string, ensuring valid UTF-8
        let linkname_str = str::from_utf8(linkname).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Convert symlink target path bytes to string
        let target_path = str::from_utf8(symlink).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;

        // Validate linkname doesn't contain path separators
        if linkname_str.contains('/') {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }

        // Get parent directory path
        let parent_path = self.fileid_to_path(dirid).await?;

        // Get root directory
        let mut root = self.root.lock().await;

        // Get parent directory - handle root directory case specially
        let parent_dir = if parent_path.is_empty() {
            &mut *root
        } else {
            match root.find_mut(&parent_path).await? {
                Some(Entity::Dir(dir)) => dir,
                Some(_) => return Err(nfsstat3::NFS3ERR_NOTDIR),
                None => return Err(nfsstat3::NFS3ERR_NOENT),
            }
        };

        // Check if symlink already exists
        if parent_dir.has_entry(linkname_str)? {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }

        // Create new symlink
        let mut symlink = SymPathLink::with_path(parent_dir.get_store().clone(), target_path)
            .map_err(nfsstat3::from)?;

        // Apply default mode if not specified
        if matches!(attr.mode, set_mode3::Void) {
            symlink
                .get_metadata_mut()
                .set_attribute(UNIX_MODE_KEY, DEFAULT_SYMLINK_MODE.to_string())
                .await
                .map_err(nfsstat3::from)?;
        }

        // Update all attributes
        Self::update_attributes(symlink.get_metadata_mut(), attr).await?;

        // Add symlink to parent directory
        parent_dir
            .put_adapted_entity(linkname_str, Entity::SymPathLink(symlink))
            .await?;

        drop(root);

        // Construct full path and ensure it is registered
        let full_path = join_path(&parent_path, linkname_str);

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered_str(&full_path).await?;

        // Get the attributes of the created symlink
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
        tracing::trace!("readlink: id: {}", id);

        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get root directory
        let root = self.root.lock().await;

        // Get the entity
        let entity = if path.is_empty() {
            return Err(nfsstat3::NFS3ERR_INVAL); // Root cannot be a symlink
        } else {
            root.find(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?
        };

        // Ensure it's a symlink and get the target path
        match entity {
            Entity::SymPathLink(symlink) => {
                let target_path = symlink.get_target_path().as_str();
                Ok(nfspath3::from(target_path.as_bytes()))
            }
            _ => Err(nfsstat3::NFS3ERR_INVAL),
        }
    }
}

impl From<FsError> for nfsstat3 {
    fn from(error: FsError) -> Self {
        tracing::error!("Converting FsError to nfsstat3: {:?}", error);
        match error {
            FsError::PathNotFound(_) => nfsstat3::NFS3ERR_NOENT,
            FsError::InvalidPathComponent(_) => nfsstat3::NFS3ERR_INVAL,
            FsError::IpldStore(_) => nfsstat3::NFS3ERR_IO,
            FsError::NotAFile(_) => nfsstat3::NFS3ERR_NOTDIR,
            FsError::NotADirectory(_) => nfsstat3::NFS3ERR_NOTDIR,
            FsError::NotASymCidLink(_) => nfsstat3::NFS3ERR_INVAL,
            FsError::NotASymPathLink(_) => nfsstat3::NFS3ERR_INVAL,
            FsError::BrokenSymCidLink(_) => nfsstat3::NFS3ERR_NOENT,
            _ => nfsstat3::NFS3ERR_IO,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

fn join_path(base_path: &str, name: &str) -> String {
    if base_path.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", base_path, name)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nfs_create_file() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Create file with default attributes
        let attr = sattr3::default();
        let (fileid, attrs) = server.create(0, &filename, attr).await.unwrap();

        // Verify file was created with correct attributes
        assert!(fileid > 0);
        assert!(matches!(attrs.ftype, ftype3::NF3REG));
        assert_eq!(attrs.mode, DEFAULT_FILE_MODE);

        // Try to create same file again - should fail
        let result = server.create(0, &filename, attr).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Try to create file with invalid filename
        let invalid_filename = filename3::from("test/invalid.txt".as_bytes());
        let result = server.create(0, &invalid_filename, attr).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Try to create file in non-existent directory
        let result = server.create(999, &filename, attr).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_nfs_create_with_custom_attributes() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Create file with custom attributes
        let mut attr = sattr3::default();
        attr.mode = set_mode3::mode(0o644);
        attr.uid = set_uid3::uid(1000);
        attr.gid = set_gid3::gid(1000);

        let (_, attrs) = server.create(0, &filename, attr).await.unwrap();

        // Verify custom attributes were set
        assert_eq!(attrs.mode, 0o644);
        assert_eq!(attrs.uid, 1000);
        assert_eq!(attrs.gid, 1000);
    }

    #[tokio::test]
    async fn test_nfs_create_exclusive() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Create file exclusively
        let fileid = server.create_exclusive(0, &filename).await.unwrap();
        assert!(fileid > 0);

        // Try to create same file again - should fail
        let result = server.create_exclusive(0, &filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Verify file has default attributes
        let attrs = server.getattr(fileid).await.unwrap();
        assert_eq!(attrs.mode, DEFAULT_FILE_MODE);
    }

    #[tokio::test]
    async fn test_nfs_lookup() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Create a file first
        let attr = sattr3::default();
        let (created_id, _) = server.create(0, &filename, attr).await.unwrap();

        // Lookup the file
        let looked_up_id = server.lookup(0, &filename).await.unwrap();
        assert_eq!(created_id, looked_up_id);

        // Try to lookup non-existent file
        let nonexistent = filename3::from("nonexistent.txt".as_bytes());
        let result = server.lookup(0, &nonexistent).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Try to lookup with invalid filename
        let invalid_filename = filename3::from("test/invalid.txt".as_bytes());
        let result = server.lookup(0, &invalid_filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));
    }

    #[tokio::test]
    async fn test_nfs_setattr() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Create a file first
        let attr = sattr3::default();
        let (fileid, _) = server.create(0, &filename, attr).await.unwrap();

        // Modify attributes
        let mut new_attr = sattr3::default();
        new_attr.mode = set_mode3::mode(0o600);
        new_attr.uid = set_uid3::uid(1001);
        new_attr.gid = set_gid3::gid(1001);

        let updated_attrs = server.setattr(fileid, new_attr).await.unwrap();

        // Verify attributes were updated
        assert_eq!(updated_attrs.mode, 0o600);
        assert_eq!(updated_attrs.uid, 1001);
        assert_eq!(updated_attrs.gid, 1001);

        // Try to setattr on non-existent file
        let result = server.setattr(999, new_attr).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_nfs_fileid_to_path() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Create a file and get its ID
        let (fileid, _) = server
            .create(0, &filename, sattr3::default())
            .await
            .unwrap();

        // Convert ID back to path
        let path = server.fileid_to_path(fileid).await.unwrap();
        assert_eq!(path, "test.txt");

        // Try with non-existent file ID
        let result = server.fileid_to_path(999).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_nfs_error_handling() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes());

        // Test handling of invalid UTF-8
        let invalid_utf8 = filename3::from(vec![0xFF, 0xFF]);
        let result = server.create(0, &invalid_utf8, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test handling of path separators in filename
        let invalid_path = filename3::from("foo/bar".as_bytes());
        let result = server.create(0, &invalid_path, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test handling of non-existent parent directory
        let result = server.create(999, &filename, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_nfs_mkdir() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let dirname = filename3::from("test_dir".as_bytes());

        // Test 1: Create directory in root
        let (fileid, attrs) = server.mkdir(0, &dirname).await.unwrap();
        assert!(fileid > 0);
        assert!(matches!(attrs.ftype, ftype3::NF3DIR));
        assert_eq!(attrs.mode, DEFAULT_DIR_MODE);

        // Test 2: Try to create same directory again - should fail
        let result = server.mkdir(0, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test 3: Create nested directory
        let nested_dirname = filename3::from("nested".as_bytes());
        let (nested_id, nested_attrs) = server.mkdir(fileid, &nested_dirname).await.unwrap();
        assert!(nested_id > 0);
        assert!(matches!(nested_attrs.ftype, ftype3::NF3DIR));
        assert_eq!(nested_attrs.mode, DEFAULT_DIR_MODE);

        // Test 4: Try to create directory with invalid name (contains '/')
        let invalid_dirname = filename3::from("invalid/name".as_bytes());
        let result = server.mkdir(0, &invalid_dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 5: Try to create directory in non-existent parent
        let result = server.mkdir(999, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test 6: Try to create directory in a file (not a directory)
        // First create a file
        let filename = filename3::from("testfile.txt".as_bytes());
        let attr = sattr3::default();
        let (file_id, _) = server.create(0, &filename, attr).await.unwrap();

        // Then try to create directory inside the file
        let result = server.mkdir(file_id, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOTDIR)));
    }

    #[tokio::test]
    async fn test_nfs_symlink() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let linkname = filename3::from("test_link".as_bytes());
        let target = nfspath3::from("target_file.txt".as_bytes());

        // Test 1: Create symlink in root
        let (fileid, attrs) = server
            .symlink(0, &linkname, &target, &sattr3::default())
            .await
            .unwrap();
        assert!(fileid > 0);
        assert!(matches!(attrs.ftype, ftype3::NF3LNK));
        assert_eq!(attrs.mode, DEFAULT_SYMLINK_MODE);

        // Test 2: Try to create same symlink again - should fail
        let result = server
            .symlink(0, &linkname, &target, &sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test 3: Create symlink in a subdirectory
        // First create a directory
        let dirname = filename3::from("test_dir".as_bytes());
        let (dir_id, _) = server.mkdir(0, &dirname).await.unwrap();

        // Then create symlink in that directory
        let nested_linkname = filename3::from("nested_link".as_bytes());
        let (nested_id, nested_attrs) = server
            .symlink(dir_id, &nested_linkname, &target, &sattr3::default())
            .await
            .unwrap();
        assert!(nested_id > 0);
        assert!(matches!(nested_attrs.ftype, ftype3::NF3LNK));
        assert_eq!(nested_attrs.mode, DEFAULT_SYMLINK_MODE);

        // Test 4: Try to create symlink with invalid name (contains '/')
        let invalid_linkname = filename3::from("invalid/name".as_bytes());
        let result = server
            .symlink(0, &invalid_linkname, &target, &sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 5: Try to create symlink in non-existent parent
        let result = server
            .symlink(999, &linkname, &target, &sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test 6: Try to create symlink in a file (not a directory)
        // First create a file
        let filename = filename3::from("testfile.txt".as_bytes());
        let (file_id, _) = server
            .create(0, &filename, sattr3::default())
            .await
            .unwrap();

        // Then try to create symlink inside the file
        let result = server
            .symlink(file_id, &linkname, &target, &sattr3::default())
            .await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOTDIR)));

        // Test 7: Create symlink with custom attributes
        let mut custom_attr = sattr3::default();
        custom_attr.mode = set_mode3::mode(0o777);
        let (custom_id, custom_attrs) = server
            .symlink(
                0,
                &filename3::from("custom_link".as_bytes()),
                &target,
                &custom_attr,
            )
            .await
            .unwrap();
        assert!(custom_id > 0);
        assert!(matches!(custom_attrs.ftype, ftype3::NF3LNK));
        assert_eq!(custom_attrs.mode, 0o777);
    }

    #[tokio::test]
    async fn test_nfs_readlink() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());
        let linkname = filename3::from("test_link".as_bytes());
        let target = nfspath3::from("target_file.txt".as_bytes());

        // Test 1: Create symlink and read its target
        let (fileid, _) = server
            .symlink(0, &linkname, &target, &sattr3::default())
            .await
            .unwrap();
        let read_target = server.readlink(fileid).await.unwrap();
        assert_eq!(read_target.as_ref(), target.as_ref());

        // Test 2: Try to read non-existent symlink
        let result = server.readlink(999).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test 3: Try to read root directory as symlink
        let result = server.readlink(0).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 4: Try to read regular file as symlink
        let filename = filename3::from("testfile.txt".as_bytes());
        let (file_id, _) = server
            .create(0, &filename, sattr3::default())
            .await
            .unwrap();
        let result = server.readlink(file_id).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 5: Try to read directory as symlink
        let dirname = filename3::from("testdir".as_bytes());
        let (dir_id, _) = server.mkdir(0, &dirname).await.unwrap();
        let result = server.readlink(dir_id).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 6: Create and read symlink with longer/nested target path
        let nested_target = nfspath3::from("path/to/nested/target.txt".as_bytes());
        let nested_linkname = filename3::from("nested_link".as_bytes());
        let (nested_id, _) = server
            .symlink(0, &nested_linkname, &nested_target, &sattr3::default())
            .await
            .unwrap();
        let read_nested_target = server.readlink(nested_id).await.unwrap();
        assert_eq!(read_nested_target.as_ref(), nested_target.as_ref());
    }

    #[tokio::test]
    async fn test_nfs_readdir() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());

        // Create test files and directories
        let file1 = filename3::from("file1.txt".as_bytes());
        let file2 = filename3::from("file2.txt".as_bytes());
        let dir1 = filename3::from("dir1".as_bytes());

        let (_, _) = server.create(0, &file1, sattr3::default()).await.unwrap();
        let (file2_id, _) = server.create(0, &file2, sattr3::default()).await.unwrap();
        let (dir1_id, _) = server.mkdir(0, &dir1).await.unwrap();

        // Test 1: Read root directory
        let result = server.readdir(0, 0, 10).await.unwrap();
        assert_eq!(result.entries.len(), 3);
        assert!(result.end); // No more entries

        // Verify entries are correct
        let mut entries = result.entries.iter();
        let _ = entries.next().unwrap();
        let _ = entries.next().unwrap();
        let _ = entries.next().unwrap();

        // Verify we have at least one file and one directory
        let has_file = result
            .entries
            .iter()
            .any(|e| matches!(e.attr.ftype, ftype3::NF3REG));
        let has_dir = result
            .entries
            .iter()
            .any(|e| matches!(e.attr.ftype, ftype3::NF3DIR));
        assert!(has_file, "Should have at least one regular file");
        assert!(has_dir, "Should have at least one directory");

        // Test 2: Pagination
        let result = server.readdir(0, 0, 2).await.unwrap();
        assert_eq!(result.entries.len(), 2);
        assert!(!result.end); // More entries exist

        let next_id = result.entries.last().unwrap().fileid;
        let result = server.readdir(0, next_id, 2).await.unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(result.end);

        // Test 3: Read from subdirectory
        let subfile = filename3::from("subfile.txt".as_bytes());
        let (subfile_id, _) = server
            .create(dir1_id, &subfile, sattr3::default())
            .await
            .unwrap();

        let result = server.readdir(dir1_id, 0, 10).await.unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(result.end);
        assert_eq!(result.entries[0].fileid, subfile_id);
        assert!(matches!(result.entries[0].attr.ftype, ftype3::NF3REG));

        // Test 4: Handle deleted entries
        server.remove(0, &file1).await.unwrap();
        let result = server.readdir(0, 0, 10).await.unwrap();
        assert_eq!(result.entries.len(), 2); // file1 should be excluded
        assert!(result.end);

        // Test 5: Error cases
        // Non-existent directory
        assert!(matches!(
            server.readdir(999, 0, 10).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Not a directory
        assert!(matches!(
            server.readdir(file2_id, 0, 10).await,
            Err(nfsstat3::NFS3ERR_NOTDIR)
        ));
    }

    #[tokio::test]
    async fn test_nfs_readdir_complex_hierarchy() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());

        // Create a complex directory structure
        let dir1 = filename3::from("dir1".as_bytes());
        let dir2 = filename3::from("dir2".as_bytes());
        let file1 = filename3::from("file1.txt".as_bytes());
        let file2 = filename3::from("file2.txt".as_bytes());

        let (dir1_id, _) = server.mkdir(0, &dir1).await.unwrap();
        let (dir2_id, _) = server.mkdir(dir1_id, &dir2).await.unwrap();
        let (_, _) = server
            .create(dir1_id, &file1, sattr3::default())
            .await
            .unwrap();
        let (_, _) = server
            .create(dir2_id, &file2, sattr3::default())
            .await
            .unwrap();

        // Test reading at different levels
        // Root directory
        let root_entries = server.readdir(0, 0, 10).await.unwrap();
        assert_eq!(root_entries.entries.len(), 1); // Only dir1

        // First level directory
        let dir1_entries = server.readdir(dir1_id, 0, 10).await.unwrap();
        assert_eq!(dir1_entries.entries.len(), 2); // dir2 and file1

        // Second level directory
        let dir2_entries = server.readdir(dir2_id, 0, 10).await.unwrap();
        assert_eq!(dir2_entries.entries.len(), 1); // Only file2

        // Test pagination at each level
        // Root directory with pagination
        let root_page1 = server.readdir(0, 0, 1).await.unwrap();
        assert_eq!(root_page1.entries.len(), 1);
        assert!(root_page1.end); // No more entries

        // Dir1 with pagination
        let dir1_page1 = server.readdir(dir1_id, 0, 1).await.unwrap();
        assert_eq!(dir1_page1.entries.len(), 1);
        assert!(!dir1_page1.end); // More entries exist

        let next_id = dir1_page1.entries[0].fileid;
        let dir1_page2 = server.readdir(dir1_id, next_id, 1).await.unwrap();
        assert_eq!(dir1_page2.entries.len(), 1);
        assert!(dir1_page2.end);
    }

    #[tokio::test]
    async fn test_nfs_readdir_empty_and_edge_cases() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());

        // Test 1: Empty directory
        let empty_dir = filename3::from("empty".as_bytes());
        let (empty_id, _) = server.mkdir(0, &empty_dir).await.unwrap();

        let result = server.readdir(empty_id, 0, 10).await.unwrap();
        assert_eq!(result.entries.len(), 0);
        assert!(result.end);

        // Test 2: Zero max_entries
        let result = server.readdir(empty_id, 0, 0).await.unwrap();
        assert_eq!(result.entries.len(), 0);
        assert!(result.end);

        // Test 3: Large max_entries
        let result = server.readdir(empty_id, 0, usize::MAX).await.unwrap();
        assert_eq!(result.entries.len(), 0);
        assert!(result.end);

        // Test 4: Invalid start_after ID
        let result = server.readdir(empty_id, 999, 10).await.unwrap();
        assert_eq!(result.entries.len(), 0);
        assert!(result.end);

        // Test 5: Directory with many entries
        let mut expected_count = 0;
        for i in 0..20 {
            let name = format!("file{}.txt", i);
            let filename = filename3::from(name.as_bytes());
            server
                .create(empty_id, &filename, sattr3::default())
                .await
                .unwrap();
            expected_count += 1;
        }

        // Read all entries
        let result = server.readdir(empty_id, 0, 100).await.unwrap();
        assert_eq!(result.entries.len(), expected_count);
        assert!(result.end);

        // Read with pagination
        let result = server.readdir(empty_id, 0, 5).await.unwrap();
        assert_eq!(result.entries.len(), 5);
        assert!(!result.end);
    }

    #[tokio::test]
    async fn test_nfs_remove() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());

        // Create test files and directories
        let file1 = filename3::from("file1.txt".as_bytes());
        let file2 = filename3::from("file2.txt".as_bytes());
        let dir1 = filename3::from("dir1".as_bytes());
        let nested_file = filename3::from("nested.txt".as_bytes());

        let (_, _) = server.create(0, &file1, sattr3::default()).await.unwrap();
        let (_, _) = server.create(0, &file2, sattr3::default()).await.unwrap();
        let (dir1_id, _) = server.mkdir(0, &dir1).await.unwrap();
        let (_, _) = server
            .create(dir1_id, &nested_file, sattr3::default())
            .await
            .unwrap();

        server.remove(0, &file1).await.unwrap();

        assert!(matches!(
            server.lookup(0, &file1).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Test 2: Remove a nested file
        server.remove(dir1_id, &nested_file).await.unwrap();
        assert!(matches!(
            server.lookup(dir1_id, &nested_file).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Test 3: Try to remove a non-existent file
        let nonexistent = filename3::from("nonexistent.txt".as_bytes());
        assert!(matches!(
            server.remove(0, &nonexistent).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Test 4: Try to remove from non-existent directory
        assert!(matches!(
            server.remove(999, &file2).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Test 5: Try to remove with invalid filename
        let invalid = filename3::from("invalid/name.txt".as_bytes());
        assert!(matches!(
            server.remove(0, &invalid).await,
            Err(nfsstat3::NFS3ERR_INVAL)
        ));
    }

    #[tokio::test]
    async fn test_nfs_rename() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());

        // Create test files and directories
        let file1 = filename3::from("file1.txt".as_bytes());
        let file2 = filename3::from("file2.txt".as_bytes());
        let dir1 = filename3::from("dir1".as_bytes());
        let dir2 = filename3::from("dir2".as_bytes());
        let nested_file = filename3::from("nested.txt".as_bytes());

        let (_, _) = server.create(0, &file1, sattr3::default()).await.unwrap();
        let (_, _) = server.create(0, &file2, sattr3::default()).await.unwrap();
        let (dir1_id, _) = server.mkdir(0, &dir1).await.unwrap();
        let (dir2_id, _) = server.mkdir(0, &dir2).await.unwrap();
        let (_, _) = server
            .create(dir1_id, &nested_file, sattr3::default())
            .await
            .unwrap();

        // Test 1: Rename a file in the same directory
        let new_name = filename3::from("renamed.txt".as_bytes());
        server.rename(0, &file1, 0, &new_name).await.unwrap();
        assert!(matches!(
            server.lookup(0, &file1).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));
        assert!(server.lookup(0, &new_name).await.is_ok());

        // Test 2: Move a file to a different directory
        let moved_name = filename3::from("moved.txt".as_bytes());
        server
            .rename(0, &file2, dir1_id, &moved_name)
            .await
            .unwrap();
        assert!(matches!(
            server.lookup(0, &file2).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));
        assert!(server.lookup(dir1_id, &moved_name).await.is_ok());

        // Test 3: Move a file between subdirectories
        let final_name = filename3::from("final.txt".as_bytes());
        server
            .rename(dir1_id, &nested_file, dir2_id, &final_name)
            .await
            .unwrap();
        assert!(matches!(
            server.lookup(dir1_id, &nested_file).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));
        assert!(server.lookup(dir2_id, &final_name).await.is_ok());

        // Test 4: Try to rename non-existent file
        let nonexistent = filename3::from("nonexistent.txt".as_bytes());
        assert!(matches!(
            server.rename(0, &nonexistent, dir1_id, &final_name).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Test 5: Try to rename to invalid filename
        let invalid = filename3::from("invalid/name.txt".as_bytes());
        assert!(matches!(
            server.rename(0, &new_name, 0, &invalid).await,
            Err(nfsstat3::NFS3ERR_INVAL)
        ));

        // Test 6: Try to rename to non-existent directory
        assert!(matches!(
            server.rename(0, &new_name, 999, &final_name).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Test 7: Verify that the file content is preserved after rename
        let root = server.root.lock().await;
        let final_path = "dir2/final.txt";
        let moved_entity = root.find(final_path).await.unwrap().unwrap();
        assert!(matches!(moved_entity, Entity::File(_)));
    }

    #[test_log::test(tokio::test)]
    async fn test_nfs_read_write() {
        let server = MemoryMonofsNFS::new(MemoryStore::default());

        // Create a test file
        let (fileid, _) = server
            .create(
                0,
                &filename3::from("test.txt".as_bytes()),
                sattr3::default(),
            )
            .await
            .unwrap();

        // Test 1: Reading from empty file
        let (empty_data, eof) = server.read(fileid, 0, 10).await.unwrap();
        assert!(empty_data.is_empty());
        assert!(eof);

        // Test 2: Write and read basic data
        let data = b"Hello, World!";
        let write_result = server.write(fileid, 0, data).await.unwrap();
        assert_eq!(write_result.size, 13); // Length of "Hello, World!"

        // Read back with different offsets
        let (full_data, eof) = server.read(fileid, 0, 13).await.unwrap();
        assert_eq!(&full_data, data);
        assert!(eof);

        // Read with offset in middle
        let (partial_data, eof) = server.read(fileid, 7, 6).await.unwrap();
        assert_eq!(&partial_data, b"World!");
        assert!(eof);

        // Read with offset at end
        let (end_data, eof) = server.read(fileid, 13, 5).await.unwrap();
        assert!(end_data.is_empty());
        assert!(eof);

        // Test 3: Append and verify with offset reads
        let append_data = b", NFS!";
        let append_result = server.write(fileid, 13, append_data).await.unwrap();
        assert_eq!(append_result.size, 19);

        // Read entire content
        let (full_data, eof) = server.read(fileid, 0, 19).await.unwrap();
        assert_eq!(&full_data, b"Hello, World!, NFS!");
        assert!(eof);

        // Read just the appended part
        let (appended_part, eof) = server.read(fileid, 13, 5).await.unwrap();
        assert_eq!(&appended_part, b", NFS");
        assert!(!eof);

        // Test 4: Overwrite in middle and verify with offset reads
        let overwrite_data = b"there";
        let overwrite_result = server.write(fileid, 7, overwrite_data).await.unwrap();
        assert_eq!(overwrite_result.size, 19);

        // Read the modified section
        let (modified_part, eof) = server.read(fileid, 7, 5).await.unwrap();
        assert_eq!(&modified_part, b"there");
        assert!(!eof);

        // Read across the modified boundary
        let (across_mod, eof) = server.read(fileid, 5, 7).await.unwrap();
        assert_eq!(&across_mod, b", there");
        assert!(!eof);

        // Test 5: Write beyond file size should fail
        let beyond_size_data = b"sparse";
        let result = server.write(fileid, 100, beyond_size_data).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 6: Edge cases
        // Read with zero count
        let (zero_count, eof) = server.read(fileid, 5, 0).await.unwrap();
        assert!(zero_count.is_empty());
        assert!(!eof);

        // Read with offset beyond file size
        let (beyond_size, eof) = server.read(fileid, 200, 10).await.unwrap();
        assert!(beyond_size.is_empty());
        assert!(eof);

        // Test 7: Error cases
        // Invalid file ID
        assert!(matches!(
            server.read(999, 0, 10).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));

        // Read from deleted file
        server
            .remove(0, &filename3::from("test.txt".as_bytes()))
            .await
            .unwrap();
        assert!(matches!(
            server.read(fileid, 0, 10).await,
            Err(nfsstat3::NFS3ERR_NOENT)
        ));
    }
}

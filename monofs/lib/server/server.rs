use std::collections::HashMap;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use intaglio::{Symbol, SymbolTable};
use libipld::Ipld;
use monoutils_store::{IpldStore, MemoryStore};
use nfsserve::nfs::{
    fattr3, fileid3, filename3, ftype3, nfspath3, nfsstat3, nfstime3, sattr3, set_atime, set_gid3,
    set_mode3, set_mtime, set_size3, set_uid3, specdata3,
};
use nfsserve::vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use typed_path::UnixPathBuf;

use crate::filesystem::{
    Dir, Entity, EntityType, File, FileInputStream, FileOutputStream, FsError, FsResult, Metadata,
    SymCidLink, SymPathLink,
};
use crate::store::FlatFsStore;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default mode for regular files (rw-r--r--)
/// Owner can read/write, group and others can read
pub const DEFAULT_FILE_MODE: u32 = 0o644;

/// Default mode for directories (rwxr-xr-x)
/// Owner has full access, group and others can read and traverse
pub const DEFAULT_DIR_MODE: u32 = 0o755;

/// Default mode for symlinks (rwxrwxrwx)
/// Permissions don't affect symlinks - they use target's permissions
pub const DEFAULT_SYMLINK_MODE: u32 = 0o777;

/// The key for the Unix mode attribute.
pub const UNIX_MODE_KEY: &str = "unix.mode";

/// The key for the Unix user ID attribute.
pub const UNIX_UID_KEY: &str = "unix.uid";

/// The key for the Unix group ID attribute.
pub const UNIX_GID_KEY: &str = "unix.gid";

/// The key for the Unix access time attribute.
pub const UNIX_ATIME_KEY: &str = "unix.atime";

/// The key for the Unix modification time attribute.
pub const UNIX_MTIME_KEY: &str = "unix.mtime";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

pub type MemoryMonofsServer = MonofsServer<MemoryStore>;
pub type DiskMonofsServer = MonofsServer<FlatFsStore>;

pub struct MonofsServer<S>
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

impl<S> MonofsServer<S>
where
    S: IpldStore + Send + Sync + 'static,
{
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
    ///
    /// ```text
    /// Input: fileid = 42
    ///
    /// ┌─────────────────────────────┐
    /// │ fileid_map                  │
    /// │ 42 => [Symbol(1),           │
    /// │       Symbol(2),            │
    /// │       Symbol(3)]            │
    /// └─────────────────────────────┘
    ///            │
    ///            ▼
    /// ┌─────────────────────────────┐
    /// │ Symbol Table Lookup         │
    /// │ Symbol(1) => "foo"          │
    /// │ Symbol(2) => "bar"          │
    /// │ Symbol(3) => "baz.txt"      │
    /// └─────────────────────────────┘
    ///            │
    ///            ▼
    /// ┌─────────────────────────────┐
    /// │ Join with "/"               │
    /// │ "foo/bar/baz.txt"           │
    /// └─────────────────────────────┘
    /// ```
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

    /// Ensures a path is registered in the path-fileid mapping system and returns its fileid.
    /// If the path is already registered, returns the existing fileid.
    /// If not, creates a new fileid and registers the bidirectional mappings.
    ///
    /// ```text
    /// Input path: "foo/bar/baz.txt"
    ///                  │
    ///                  ▼
    /// ┌─────────────────────────────┐
    /// │ Split into segments         │
    /// │  ["foo", "bar", "baz.txt"]  │
    /// └─────────────────────────────┘
    ///                  │
    ///                  ▼
    /// ┌─────────────────────────────┐
    /// │ Convert to Symbols          │
    /// │  [Symbol(1),                │
    /// │   Symbol(2),                │
    /// │   Symbol(3)]                │
    /// └─────────────────────────────┘
    ///                  │
    ///                  ▼
    /// ┌─────────────────────────────┐
    /// │ Check if path exists        │
    /// │ If yes: return existing id  │
    /// │ If no:  create new mapping  │
    /// └─────────────────────────────┘
    /// ```
    async fn ensure_path_registered(&self, path: &str) -> Result<fileid3, nfsstat3> {
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

        // Check if path already exists in path_to_fileid_map
        let mut path_to_fileid_map = self.path_to_fileid_map.lock().await;
        if let Some(&existing_id) = path_to_fileid_map.get(&path_symbols) {
            return Ok(existing_id);
        }

        // If path doesn't exist, create new mapping
        let fileid = self.next_fileid();
        let mut fileid_to_path_map = self.fileid_to_path_map.lock().await;

        fileid_to_path_map.insert(fileid, path_symbols.clone());
        path_to_fileid_map.insert(path_symbols, fileid);

        Ok(fileid)
    }

    /// Helper method to update attributes on an entity's metadata
    async fn update_attributes(metadata: &mut Metadata<S>, attr: &sattr3) -> Result<(), nfsstat3> {
        // Update mode
        match attr.mode {
            set_mode3::Void => {}
            set_mode3::mode(mode) => {
                metadata
                    .set_attribute(UNIX_MODE_KEY, mode.to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update uid
        match attr.uid {
            set_uid3::Void => {}
            set_uid3::uid(uid) => {
                metadata
                    .set_attribute(UNIX_UID_KEY, uid.to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update gid
        match attr.gid {
            set_gid3::Void => {}
            set_gid3::gid(gid) => {
                metadata
                    .set_attribute(UNIX_GID_KEY, gid.to_string())
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
                    .set_attribute(UNIX_ATIME_KEY, now.timestamp().to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }
            set_atime::SET_TO_CLIENT_TIME(atime) => {
                metadata
                    .set_attribute(UNIX_ATIME_KEY, atime.seconds.to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        // Update mtime
        match attr.mtime {
            set_mtime::DONT_CHANGE => {}
            set_mtime::SET_TO_SERVER_TIME => {
                let now = Utc::now();
                metadata
                    .set_attribute(UNIX_MTIME_KEY, now.timestamp().to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }
            set_mtime::SET_TO_CLIENT_TIME(mtime) => {
                metadata
                    .set_attribute(UNIX_MTIME_KEY, mtime.seconds.to_string())
                    .await
                    .map_err(nfsstat3::from)?;
            }
        }

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl<S> NFSFileSystem for MonofsServer<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    fn root_dir(&self) -> fileid3 {
        0
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
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
        if !parent_dir.has_entry(filename_str)? {
            return Err(nfsstat3::NFS3ERR_NOENT);
        }

        drop(root);

        // Construct full path
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Ensure path is registered and get its fileid
        self.ensure_path_registered(&full_path).await
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        // Get path from fileid
        let path = self.fileid_to_path(id).await?;

        // Get root directory
        let root = self.root.lock().await;

        // Get metadata
        let metadata = if path.is_empty() {
            root.get_metadata()
        } else {
            let entity = root.find(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?;
            entity.get_metadata()
        };

        // Convert to NFS attributes
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
                .and_then(|ipld| match ipld {
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
                .and_then(|ipld| match ipld {
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
                .unwrap_or(0),
            // Default gid is 0 (root) if not set or invalid
            gid: metadata
                .get_attribute(UNIX_GID_KEY)
                .await
                .map_err(nfsstat3::from)?
                .and_then(|ipld| match ipld {
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
                .unwrap_or(0),
            size: 0, // TODO: Size is not stored in metadata
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
                seconds: metadata
                    .get_modified_at()
                    .map(|t| t.timestamp())
                    .unwrap_or_else(|| metadata.get_created_at().timestamp())
                    as u32,
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

        // Get root directory
        let mut root = self.root.lock().await;

        // Get metadata
        let metadata = if path.is_empty() {
            root.get_metadata_mut()
        } else {
            let entity = root.find_mut(&path).await?.ok_or(nfsstat3::NFS3ERR_NOENT)?;
            entity.get_metadata_mut()
        };

        // Update all attributes
        Self::update_attributes(metadata, &setattr).await?;

        drop(root);

        // Return updated attributes
        self.getattr(id).await
    }

    async fn read(
        &self,
        id: fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        todo!()
    }

    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        todo!()
    }

    async fn create(
        &self,
        dirid: fileid3,
        filename: &filename3,
        attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
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
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered(&full_path).await?;

        // Get the attributes of the created file
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn create_exclusive(
        &self,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
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
        let full_path = if parent_path.is_empty() {
            filename_str.to_string()
        } else {
            format!("{}/{}", parent_path, filename_str)
        };

        // Ensure path is registered and get its fileid
        self.ensure_path_registered(&full_path).await
    }

    async fn mkdir(
        &self,
        dirid: fileid3,
        dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
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
        let full_path = if parent_path.is_empty() {
            dirname_str.to_string()
        } else {
            format!("{}/{}", parent_path, dirname_str)
        };

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered(&full_path).await?;

        // Get the attributes of the created directory
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        todo!()
    }

    async fn rename(
        &self,
        from_dirid: fileid3,
        from_filename: &filename3,
        to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        Ok(())
    }

    async fn readdir(
        &self,
        dirid: fileid3,
        start_after: fileid3,
        max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        todo!()
    }

    async fn symlink(
        &self,
        dirid: fileid3,
        linkname: &filename3,
        symlink: &nfspath3,
        attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
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
        parent_dir.put_entity(linkname_str, Entity::SymPathLink(symlink))?;

        drop(root);

        // Construct full path and ensure it is registered
        let full_path = if parent_path.is_empty() {
            linkname_str.to_string()
        } else {
            format!("{}/{}", parent_path, linkname_str)
        };

        // Ensure path is registered and get its fileid
        let fileid = self.ensure_path_registered(&full_path).await?;

        // Get the attributes of the created symlink
        let attrs = self.getattr(fileid).await?;

        Ok((fileid, attrs))
    }

    async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
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
                Ok(nfspath3::from(target_path.as_bytes().to_vec()))
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
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nfs_create_file() {
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

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
        let invalid_filename = filename3::from("test/invalid.txt".as_bytes().to_vec());
        let result = server.create(0, &invalid_filename, attr).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Try to create file in non-existent directory
        let result = server.create(999, &filename, attr).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_nfs_create_with_custom_attributes() {
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

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
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

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
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

        // Create a file first
        let attr = sattr3::default();
        let (created_id, _) = server.create(0, &filename, attr).await.unwrap();

        // Lookup the file
        let looked_up_id = server.lookup(0, &filename).await.unwrap();
        assert_eq!(created_id, looked_up_id);

        // Try to lookup non-existent file
        let nonexistent = filename3::from("nonexistent.txt".as_bytes().to_vec());
        let result = server.lookup(0, &nonexistent).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Try to lookup with invalid filename
        let invalid_filename = filename3::from("test/invalid.txt".as_bytes().to_vec());
        let result = server.lookup(0, &invalid_filename).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));
    }

    #[tokio::test]
    async fn test_nfs_setattr() {
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

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
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

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
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let filename = filename3::from("test.txt".as_bytes().to_vec());

        // Test handling of invalid UTF-8
        let invalid_utf8 = filename3::from(vec![0xFF, 0xFF]);
        let result = server.create(0, &invalid_utf8, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test handling of path separators in filename
        let invalid_path = filename3::from("foo/bar".as_bytes().to_vec());
        let result = server.create(0, &invalid_path, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test handling of non-existent parent directory
        let result = server.create(999, &filename, sattr3::default()).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));
    }

    #[tokio::test]
    async fn test_nfs_mkdir() {
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let dirname = filename3::from("test_dir".as_bytes().to_vec());

        // Test 1: Create directory in root
        let (fileid, attrs) = server.mkdir(0, &dirname).await.unwrap();
        assert!(fileid > 0);
        assert!(matches!(attrs.ftype, ftype3::NF3DIR));
        assert_eq!(attrs.mode, DEFAULT_DIR_MODE);

        // Test 2: Try to create same directory again - should fail
        let result = server.mkdir(0, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));

        // Test 3: Create nested directory
        let nested_dirname = filename3::from("nested".as_bytes().to_vec());
        let (nested_id, nested_attrs) = server.mkdir(fileid, &nested_dirname).await.unwrap();
        assert!(nested_id > 0);
        assert!(matches!(nested_attrs.ftype, ftype3::NF3DIR));
        assert_eq!(nested_attrs.mode, DEFAULT_DIR_MODE);

        // Test 4: Try to create directory with invalid name (contains '/')
        let invalid_dirname = filename3::from("invalid/name".as_bytes().to_vec());
        let result = server.mkdir(0, &invalid_dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 5: Try to create directory in non-existent parent
        let result = server.mkdir(999, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOENT)));

        // Test 6: Try to create directory in a file (not a directory)
        // First create a file
        let filename = filename3::from("testfile.txt".as_bytes().to_vec());
        let attr = sattr3::default();
        let (file_id, _) = server.create(0, &filename, attr).await.unwrap();

        // Then try to create directory inside the file
        let result = server.mkdir(file_id, &dirname).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOTDIR)));
    }

    #[tokio::test]
    async fn test_nfs_symlink() {
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let linkname = filename3::from("test_link".as_bytes().to_vec());
        let target = nfspath3::from("target_file.txt".as_bytes().to_vec());

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
        let dirname = filename3::from("test_dir".as_bytes().to_vec());
        let (dir_id, _) = server.mkdir(0, &dirname).await.unwrap();

        // Then create symlink in that directory
        let nested_linkname = filename3::from("nested_link".as_bytes().to_vec());
        let (nested_id, nested_attrs) = server
            .symlink(dir_id, &nested_linkname, &target, &sattr3::default())
            .await
            .unwrap();
        assert!(nested_id > 0);
        assert!(matches!(nested_attrs.ftype, ftype3::NF3LNK));
        assert_eq!(nested_attrs.mode, DEFAULT_SYMLINK_MODE);

        // Test 4: Try to create symlink with invalid name (contains '/')
        let invalid_linkname = filename3::from("invalid/name".as_bytes().to_vec());
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
        let filename = filename3::from("testfile.txt".as_bytes().to_vec());
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
                &filename3::from("custom_link".as_bytes().to_vec()),
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
        let server = MemoryMonofsServer::new(MemoryStore::default());
        let linkname = filename3::from("test_link".as_bytes().to_vec());
        let target = nfspath3::from("target_file.txt".as_bytes().to_vec());

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
        let filename = filename3::from("testfile.txt".as_bytes().to_vec());
        let (file_id, _) = server
            .create(0, &filename, sattr3::default())
            .await
            .unwrap();
        let result = server.readlink(file_id).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 5: Try to read directory as symlink
        let dirname = filename3::from("testdir".as_bytes().to_vec());
        let (dir_id, _) = server.mkdir(0, &dirname).await.unwrap();
        let result = server.readlink(dir_id).await;
        assert!(matches!(result, Err(nfsstat3::NFS3ERR_INVAL)));

        // Test 6: Create and read symlink with longer/nested target path
        let nested_target = nfspath3::from("path/to/nested/target.txt".as_bytes().to_vec());
        let nested_linkname = filename3::from("nested_link".as_bytes().to_vec());
        let (nested_id, _) = server
            .symlink(0, &nested_linkname, &nested_target, &sattr3::default())
            .await
            .unwrap();
        let read_nested_target = server.readlink(nested_id).await.unwrap();
        assert_eq!(read_nested_target.as_ref(), nested_target.as_ref());
    }
}

use async_recursion::async_recursion;
use chrono::{TimeZone, Utc};
use ipldstore::{
    ipld::{cid::Cid, ipld::Ipld},
    IpldStore,
};
use monofs::filesystem::{
    Dir, Entity, File, Metadata, SymPathLink, UNIX_ATIME_KEY, UNIX_GID_KEY, UNIX_MODE_KEY,
    UNIX_MTIME_KEY, UNIX_UID_KEY,
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};
use tokio::{fs::File as TokioFile, io::BufReader};

use crate::MonocoreResult;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The opaque directory marker file name used in OCI layers.
pub const OPAQUE_WHITEOUT_MARKER: &str = ".wh..wh..opq";

/// The prefix for whiteout files in OCI layers.
pub const WHITEOUT_PREFIX: &str = ".wh.";

//--------------------------------------------------------------------------------------------------
// Structs
//--------------------------------------------------------------------------------------------------

/// RAII guard that temporarily changes file permissions and restores them when dropped
struct PermissionGuard {
    path: std::path::PathBuf,
    original_mode: u32,
}

impl PermissionGuard {
    /// Creates a new guard that temporarily adds the given mode bits to the file permissions
    fn new(path: impl AsRef<Path>, mode_to_add: u32) -> MonocoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let metadata = fs::metadata(&path)?;
        let original_mode = metadata.permissions().mode();

        // Update permissions
        let mut perms = metadata.permissions();
        perms.set_mode(original_mode | mode_to_add);
        fs::set_permissions(&path, perms)?;

        Ok(Self {
            path,
            original_mode,
        })
    }
}

impl Drop for PermissionGuard {
    fn drop(&mut self) {
        // Attempt to restore original permissions, ignore errors during drop
        if let Ok(mut perms) = fs::metadata(&self.path).and_then(|m| Ok(m.permissions())) {
            perms.set_mode(self.original_mode);
            let _ = fs::set_permissions(&self.path, perms);
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Creates a monofs filesystem from a directory path, preserving Unix metadata and OCI layer semantics.
///
/// Handles regular files, directories, and symlinks. Preserves Unix metadata (mode, uid, gid,
/// timestamps) in extended attributes. Supports OCI layer features like whiteouts and opaque
/// directories.
///
/// ## Permission Handling
///
/// This function may temporarily modify file permissions to ensure access to restricted
/// files/directories during the copy process. These modifications are guaranteed to be reverted
/// after access, even if errors occur. The original permissions are both restored on disk and
/// preserved in the resulting monofs filesystem.
///
/// ## Arguments
///
/// * `layer_path` - Source directory path
/// * `store` - IPLD store for filesystem data
///
/// ## Returns
///
/// Returns a tuple containing:
///
/// * The CID of the root directory
/// * The root directory
pub async fn create_monofs_from_oci_layer<S>(
    layer_path: impl AsRef<Path>,
    store: S,
) -> MonocoreResult<(Cid, Dir<S>)>
where
    S: IpldStore + Clone + Send + Sync + 'static,
{
    let mut root_dir = Dir::new(store);
    create_entries(&mut root_dir, layer_path.as_ref()).await?;
    let root_cid = root_dir.checkpoint().await?;
    Ok((root_cid, root_dir))
}

/// Merges multiple monofs layers into a single layer, following OCI layer semantics in a bottom-up manner.
///
/// This function takes a slice of layers and merges them according to OCI image layer semantics,
/// where each layer can contain whiteout files and opaque directories that affect the visibility
/// of files in lower layers.
///
/// ## Layer Order
///
/// The layers are expected to be provided in bottom-up order, where the first layer is the
/// oldest/base layer and subsequent layers are increasingly newer layers that override files
/// from lower layers.
///
/// For OCI layer semantics:
/// - Regular whiteout files (`.wh.filename`) hide the corresponding file/directory in lower layers
/// - Opaque whiteout markers (`.wh..wh..opq`) in a directory hide all contents from lower layers
/// - Non-whiteout files and directories from upper layers take precedence over lower layers
///
/// ## Returns
///
/// Returns a tuple containing:
///
/// * The CID of the merged root directory
/// * The merged root directory
///
/// ## Example
///
/// ```ignore
/// use monofs::filesystem::Dir;
/// use ipldstore::MemoryStore;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = MemoryStore::default();
/// let layer1 = /* base layer */;
/// let layer2 = /* middle layer */;
/// let layer3 = /* top layer */;
///
/// // Merge layers from bottom (base) to top (overlay)
/// let (merged_cid, merged_dir) = merge_oci_based_monofs_layers(vec![layer1, layer2, layer3], store.clone()).await?;
/// # Ok(())
/// # }
/// ```
#[async_recursion]
pub async fn merge_oci_based_monofs_layers<S>(
    layers: Vec<Dir<S>>,
    store: S,
) -> MonocoreResult<(Cid, Dir<S>)>
where
    S: IpldStore + Clone + Send + Sync + 'static,
{
    let mut merged = Dir::new(store);

    // Process layers in provided bottom-up order
    for layer in layers {
        // Collect all entries from the current layer
        let entries: Vec<_> = layer.get_entries().collect();

        // First, process all whiteout entries
        for (name, _) in entries
            .iter()
            .filter(|(name, _)| name.as_str().starts_with(WHITEOUT_PREFIX))
        {
            let name_str = name.as_str();
            if name_str == OPAQUE_WHITEOUT_MARKER {
                // Opaque marker: skip
                continue;
            }
            let whiteout_target = name_str.strip_prefix(WHITEOUT_PREFIX).unwrap();
            if merged.has_entry(whiteout_target)? {
                merged.remove_entry(whiteout_target)?;
            }
        }

        // Then, process all non-whiteout entries
        for (name, link) in entries
            .iter()
            .filter(|(name, _)| !name.as_str().starts_with(WHITEOUT_PREFIX))
        {
            let name_str = name.as_str();

            // Resolve the entity from the link
            let entity = link.resolve_entity(merged.get_store().clone()).await?;
            match entity {
                Entity::Dir(dir) => {
                    let is_opaque = dir.has_entry(OPAQUE_WHITEOUT_MARKER)?;
                    if is_opaque {
                        // If opaque, add the directory directly without merging
                        merged.put_adapted_dir(name_str, dir.clone()).await?;
                    } else if let Some(existing_dir) = merged.get_dir(name_str).await? {
                        // If directory exists, recursively merge: base comes first, overlay second
                        let (_, merged_subdir) = Box::pin(merge_oci_based_monofs_layers(
                            vec![existing_dir.clone(), dir.clone()],
                            merged.get_store().clone(),
                        ))
                        .await?;
                        merged.put_adapted_dir(name_str, merged_subdir).await?;
                    } else {
                        // If directory doesn't exist, add it
                        merged.put_adapted_dir(name_str, dir.clone()).await?;
                    }
                }
                non_dir => {
                    // For non-directory entities, simply add or replace
                    merged.put_adapted_entity(name_str, non_dir.clone()).await?;
                }
            }
        }
    }

    // Create a checkpoint to get the final CID
    let cid = merged.checkpoint().await?;
    Ok((cid, merged))
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

#[async_recursion]
async fn create_entries<S>(dir: &mut Dir<S>, path: &Path) -> MonocoreResult<()>
where
    S: IpldStore + Clone + Send + Sync + 'static,
{
    // First try to read the directory with current permissions
    let entries_result = fs::read_dir(path);

    // If we can't read the directory due to permissions, temporarily update them
    let entries = match entries_result {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Create permission guard that adds rwx for owner
            let _guard = PermissionGuard::new(path, 0o700)?;
            fs::read_dir(path)?
        }
        Err(e) => return Err(e.into()),
    };

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let entry_path = entry.path();

        // Get metadata without following symlinks for all entry types
        let metadata = match fs::symlink_metadata(&entry_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // If we can't access metadata, try with elevated permissions
                let _guard = PermissionGuard::new(&entry_path, 0o700)?;
                fs::symlink_metadata(&entry_path)?
            }
            Err(e) => return Err(e.into()),
        };

        if file_type.is_dir() {
            // Create a new directory
            let mut new_dir = Dir::new(dir.get_store().clone());

            // Set directory metadata
            set_metadata(new_dir.get_metadata_mut(), &metadata).await?;

            // Recursively process subdirectory
            create_entries(&mut new_dir, &entry_path).await?;

            // Add directory to parent
            dir.put_adapted_dir(&file_name, new_dir).await?;
        } else if file_type.is_file() {
            // Create a new file based on whether it's empty or not
            let mut new_file = if metadata.len() == 0 {
                // Create empty file
                File::new(dir.get_store().clone())
            } else {
                // Try to open and read the file, updating permissions if needed
                let file = match TokioFile::open(&entry_path).await {
                    Ok(f) => f,
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        // Create permission guard that adds read permission for owner
                        let _guard = PermissionGuard::new(&entry_path, 0o400)?;
                        TokioFile::open(&entry_path).await?
                    }
                    Err(e) => return Err(e.into()),
                };

                let reader = BufReader::new(file);
                File::with_content(dir.get_store().clone(), reader).await?
            };

            // Set file metadata
            set_metadata(new_file.get_metadata_mut(), &metadata).await?;

            // Add file to parent
            dir.put_adapted_file(&file_name, new_file).await?;
        } else if file_type.is_symlink() {
            // Read symlink target
            let target_path = fs::read_link(&entry_path)?;
            let target_str = target_path.to_string_lossy().into_owned();

            // Create symlink
            let mut symlink = SymPathLink::with_path(dir.get_store().clone(), target_str)?;

            // Set symlink metadata using the metadata we already obtained
            set_metadata(symlink.get_metadata_mut(), &metadata).await?;

            // Add symlink to parent
            dir.put_adapted_sympathlink(&file_name, symlink).await?;
        }
    }

    Ok(())
}

async fn set_metadata<S>(
    metadata: &mut Metadata<S>,
    fs_metadata: &fs::Metadata,
) -> MonocoreResult<()>
where
    S: IpldStore + Send + Sync,
{
    // Set Unix permissions
    let raw_mode = fs_metadata.mode();
    let permission_bits = raw_mode & 0o777;
    metadata
        .set_attribute(UNIX_MODE_KEY, Ipld::Integer(permission_bits as i128))
        .await?;

    // Set ownership
    metadata
        .set_attribute(UNIX_UID_KEY, Ipld::Integer(fs_metadata.uid() as i128))
        .await?;
    metadata
        .set_attribute(UNIX_GID_KEY, Ipld::Integer(fs_metadata.gid() as i128))
        .await?;

    // Set timestamps
    let atime = Utc
        .timestamp_opt(fs_metadata.atime(), 0)
        .single()
        .unwrap_or_else(|| Utc::now());
    let mtime = Utc
        .timestamp_opt(fs_metadata.mtime(), 0)
        .single()
        .unwrap_or_else(|| Utc::now());

    metadata
        .set_attribute(UNIX_ATIME_KEY, Ipld::String(atime.to_rfc3339()))
        .await?;
    metadata
        .set_attribute(UNIX_MTIME_KEY, Ipld::String(mtime.to_rfc3339()))
        .await?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ipldstore::MemoryStore;

    use helper::*;

    #[test_log::test(tokio::test)]
    async fn test_rootfs_create_monofs_from_oci_layer() -> anyhow::Result<()> {
        let temp_dir = setup_test_filesystem().await?;
        let store = MemoryStore::default();

        // Create monofs filesystem from the temp directory
        let (_, root_dir) = create_monofs_from_oci_layer(temp_dir.path(), store.clone()).await?;

        // Verify regular directory
        let regular_dir = root_dir
            .get_dir("regular_dir")
            .await?
            .expect("regular_dir should exist");
        tracing::info!("regular_dir");
        verify_metadata(
            regular_dir.get_metadata(),
            &fs::metadata(temp_dir.path().join("regular_dir"))?,
        )
        .await?;

        // Verify regular file
        let regular_file = regular_dir
            .get_file("regular_file.txt")
            .await?
            .expect("regular_file.txt should exist");
        tracing::info!("regular_dir/regular_file.txt");
        verify_metadata(
            regular_file.get_metadata(),
            &fs::metadata(temp_dir.path().join("regular_dir/regular_file.txt"))?,
        )
        .await?;

        // Verify executable
        let executable = regular_dir
            .get_file("executable.sh")
            .await?
            .expect("executable.sh should exist");
        tracing::info!("regular_dir/executable.sh");
        verify_metadata(
            executable.get_metadata(),
            &fs::metadata(temp_dir.path().join("regular_dir/executable.sh"))?,
        )
        .await?;

        // Verify symlink
        let symlink_link = regular_dir
            .get_entry("symlink")?
            .expect("symlink should exist");
        let symlink_entity = symlink_link.resolve_entity(store.clone()).await?;
        tracing::info!("regular_dir/symlink");
        verify_metadata(
            symlink_entity.get_metadata(),
            &fs::symlink_metadata(temp_dir.path().join("regular_dir/symlink"))?,
        )
        .await?;

        // Verify opaque directory
        let opaque_dir = root_dir
            .get_dir("opaque_dir")
            .await?
            .expect("opaque_dir should exist");
        tracing::info!("opaque_dir");
        verify_metadata(
            opaque_dir.get_metadata(),
            &fs::metadata(temp_dir.path().join("opaque_dir"))?,
        )
        .await?;

        // Verify whiteout files
        let whiteouts_dir = root_dir
            .get_dir("whiteouts")
            .await?
            .expect("whiteouts dir should exist");
        tracing::info!("whiteouts");
        assert!(whiteouts_dir
            .get_file(&format!("{}{}", WHITEOUT_PREFIX, "deleted_file"))
            .await?
            .is_some());
        assert!(whiteouts_dir
            .get_file(&format!("{}{}", WHITEOUT_PREFIX, "deleted_dir"))
            .await?
            .is_some());
        tracing::info!("whiteouts/deleted_file");
        tracing::info!("whiteouts/deleted_dir");

        // Verify restricted-permission directory and file
        let restricted_dir = root_dir
            .get_dir("restricted_dir")
            .await?
            .expect("restricted_dir should exist");
        tracing::info!("restricted_dir");
        verify_metadata(
            restricted_dir.get_metadata(),
            &fs::metadata(temp_dir.path().join("restricted_dir"))?,
        )
        .await?;

        let restricted_file = restricted_dir
            .get_file("restricted_file.txt")
            .await?
            .expect("restricted_file.txt should exist");
        tracing::info!("restricted_dir/restricted_file.txt");
        verify_metadata(
            restricted_file.get_metadata(),
            &fs::metadata(temp_dir.path().join("restricted_dir/restricted_file.txt"))?,
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_rootfs_merge_oci_based_monofs_layers() -> anyhow::Result<()> {
        let store = MemoryStore::default();

        // Create the three layers
        let layer1 = helper::create_base_layer(store.clone()).await?;
        let layer2 = helper::create_middle_layer(store.clone()).await?;
        let layer3 = helper::create_top_layer(store.clone()).await?;

        // Merge layers from bottom (base) to top (overlay)
        let (_, merged) =
            merge_oci_based_monofs_layers(vec![layer1, layer2, layer3], store.clone()).await?;

        // Verify the merged result

        // 1. Verify new files from top layer exist
        helper::verify_file_content(&merged, "new_dir/new_file.txt", "new file").await?;
        helper::verify_file_content(&merged, "app_dir/file2.txt", "modified file2").await?;

        // 2. Verify middle layer changes
        helper::verify_file_content(&merged, "app_dir/file1.txt", "modified file1").await?;
        helper::verify_file_content(&merged, "whiteout_dir/new_file.txt", "new file in overlay")
            .await?;

        // 3. Verify whiteout effects
        assert!(merged
            .get_dir("app_dir")
            .await?
            .unwrap()
            .get_file("to_be_deleted.txt")
            .await?
            .is_none());

        // 4. Verify opaque directory effects (persist.txt from base layer should be hidden)
        assert!(merged
            .get_dir("whiteout_dir")
            .await?
            .unwrap()
            .get_file("persist.txt")
            .await?
            .is_none());

        Ok(())
    }
}

#[cfg(test)]
mod helper {
    use ipldstore::IpldStoreSeekable;
    use std::{ops::Deref, os::unix::fs::PermissionsExt};
    use tokio::io::AsyncReadExt;

    use crate::{error::MonocoreError, utils};

    use super::*;
    use nix::unistd::{self, Gid, Uid};
    use tempfile::TempDir;

    pub const TEST_UID: u32 = 1000;
    pub const TEST_GID: u32 = 1000;

    /// The test filesystem is structured as follows:
    ///
    /// ```txt
    /// temp_dir/
    /// ├── regular_dir/
    /// │   ├── regular_file.txt (rw-r--r--)
    /// │   ├── executable.sh (rwxr-xr-x)
    /// │   ├── owned_file.txt (uid: 1000, gid: 1000)
    /// │   ├── symlink -> regular_file.txt
    /// │   └── nested_dir/
    /// ├── opaque_dir/
    /// │   ├── .wh..wh..opq
    /// │   └── regular_file.txt
    /// ├── restricted_dir/ (--x-------)
    /// │   └── restricted_file.txt (r---------)
    /// └── whiteouts/
    ///     ├── .wh.deleted_file
    ///     └── .wh.deleted_dir
    /// ```
    pub async fn setup_test_filesystem() -> anyhow::Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create regular directory with nested structure
        let dir_path = root.join("regular_dir");
        fs::create_dir(&dir_path)?;
        fs::create_dir(dir_path.join("nested_dir"))?;

        // Create regular file with content and specific permissions (rw-r--r--)
        let file_path = dir_path.join("regular_file.txt");
        fs::write(&file_path, "Hello, World!")?;
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644))?;

        // Create executable file (rwxr-xr-x)
        let exec_path = dir_path.join("executable.sh");
        fs::write(&exec_path, "#!/bin/sh\necho 'Hello'")?;
        fs::set_permissions(&exec_path, fs::Permissions::from_mode(0o755))?;

        // Create symlink
        let symlink_path = dir_path.join("symlink");
        std::os::unix::fs::symlink("regular_file.txt", &symlink_path)?;

        // Create file with specific owner (if possible)
        let owned_file_path = dir_path.join("owned_file.txt");
        fs::write(&owned_file_path, "Owned content")?;
        if unistd::geteuid().is_root() {
            unistd::chown(
                &owned_file_path,
                Some(Uid::from_raw(TEST_UID)),
                Some(Gid::from_raw(TEST_GID)),
            )?;
        }

        // Create opaque directory (used in OCI layers)
        let opaque_dir = root.join("opaque_dir");
        fs::create_dir(&opaque_dir)?;
        // Create the opaque marker file (OCI standard)
        fs::File::create(opaque_dir.join(OPAQUE_WHITEOUT_MARKER))?;
        fs::write(
            opaque_dir.join("regular_file.txt"),
            "File in opaque directory",
        )?;

        // Create whiteout files (used in OCI layers)
        let whiteouts_dir = root.join("whiteouts");
        fs::create_dir(&whiteouts_dir)?;
        // Whiteout a file
        fs::File::create(whiteouts_dir.join(format!("{}{}", WHITEOUT_PREFIX, "deleted_file")))?;
        // Whiteout a directory
        fs::File::create(whiteouts_dir.join(format!("{}{}", WHITEOUT_PREFIX, "deleted_dir")))?;

        // Create directory and file with restricted permissions
        let restricted_dir = root.join("restricted_dir");
        fs::create_dir(&restricted_dir)?;
        let restricted_file = restricted_dir.join("restricted_file.txt");
        fs::write(&restricted_file, "Content in restricted-permission file")?;

        // Set restricted permissions: directory with only execute (--x-------) and file with only read (r---------)
        fs::set_permissions(&restricted_file, fs::Permissions::from_mode(0o400))?;
        fs::set_permissions(&restricted_dir, fs::Permissions::from_mode(0o100))?;

        Ok(temp_dir)
    }

    pub async fn verify_metadata<S: IpldStore + Clone + Send + Sync>(
        entity: &Metadata<S>,
        fs_metadata: &fs::Metadata,
    ) -> anyhow::Result<()> {
        // Verify mode
        let stored_mode = entity
            .get_attribute(UNIX_MODE_KEY)
            .await?
            .expect("mode should exist");
        let stored_mode_decimal: u32 = stored_mode.deref().clone().try_into().unwrap();
        let raw_mode = fs_metadata.mode();
        let fs_mode_decimal = raw_mode & 0o777;
        tracing::info!(
            "stored_mode_decimal: {}, fs_mode_decimal: {}",
            utils::format_mode(stored_mode_decimal),
            utils::format_mode(fs_mode_decimal)
        );
        assert_eq!(
            stored_mode_decimal, fs_mode_decimal,
            "Mode mismatch - stored: {:o}, fs: {:o}",
            stored_mode_decimal, fs_mode_decimal
        );

        // Verify ownership
        let stored_uid: u32 = entity
            .get_attribute(UNIX_UID_KEY)
            .await?
            .expect("uid should exist")
            .deref()
            .clone()
            .try_into()
            .unwrap();
        let stored_gid: u32 = entity
            .get_attribute(UNIX_GID_KEY)
            .await?
            .expect("gid should exist")
            .deref()
            .clone()
            .try_into()
            .unwrap();

        assert_eq!(stored_uid, fs_metadata.uid());
        assert_eq!(stored_gid, fs_metadata.gid());

        // Verify timestamps
        let stored_atime: String = entity
            .get_attribute(UNIX_ATIME_KEY)
            .await?
            .expect("atime should exist")
            .deref()
            .clone()
            .try_into()
            .unwrap();
        let stored_mtime: String = entity
            .get_attribute(UNIX_MTIME_KEY)
            .await?
            .expect("mtime should exist")
            .deref()
            .clone()
            .try_into()
            .unwrap();

        let atime = Utc
            .timestamp_opt(fs_metadata.atime(), 0)
            .single()
            .unwrap_or_else(|| Utc::now());
        let mtime = Utc
            .timestamp_opt(fs_metadata.mtime(), 0)
            .single()
            .unwrap_or_else(|| Utc::now());

        assert_eq!(stored_atime, atime.to_rfc3339());
        assert_eq!(stored_mtime, mtime.to_rfc3339());

        Ok(())
    }

    /// Creates a base layer with initial files and directories.
    /// Layer structure:
    /// ```text
    /// root_dir/
    /// ├── app_dir/
    /// │   ├── file1.txt ("base file1")
    /// │   ├── file2.txt ("base file2")
    /// │   └── to_be_deleted.txt ("will be deleted")
    /// └── whiteout_dir/
    ///     └── persist.txt ("persist")
    /// ```
    pub async fn create_base_layer<S>(store: S) -> MonocoreResult<Dir<S>>
    where
        S: IpldStore + Clone + Send + Sync + 'static,
    {
        let mut root = Dir::new(store.clone());

        // Create app_dir with files
        let mut app_dir = Dir::new(store.clone());
        let file1 = File::with_content(store.clone(), b"base file1".as_slice()).await?;
        app_dir.put_adapted_file("file1.txt", file1).await?;

        let file2 = File::with_content(store.clone(), b"base file2".as_slice()).await?;
        app_dir.put_adapted_file("file2.txt", file2).await?;

        let to_delete = File::with_content(store.clone(), b"will be deleted".as_slice()).await?;
        app_dir
            .put_adapted_file("to_be_deleted.txt", to_delete)
            .await?;

        root.put_adapted_dir("app_dir", app_dir).await?;

        // Create whiteout_dir with file
        let mut whiteout_dir = Dir::new(store.clone());
        let persist_file = File::with_content(store.clone(), b"persist".as_slice()).await?;
        whiteout_dir
            .put_adapted_file("persist.txt", persist_file)
            .await?;

        root.put_adapted_dir("whiteout_dir", whiteout_dir).await?;

        Ok(root)
    }

    /// Creates a middle layer with whiteouts and an opaque directory.
    /// Layer structure:
    /// ```text
    /// root_dir/
    /// ├── app_dir/
    /// │   ├── .wh.to_be_deleted.txt (whiteout)
    /// │   └── file1.txt ("modified file1")
    /// └── whiteout_dir/
    ///     ├── .wh..wh..opq (opaque marker)
    ///     └── new_file.txt ("new file in overlay")
    /// ```
    pub async fn create_middle_layer<S>(store: S) -> MonocoreResult<Dir<S>>
    where
        S: IpldStore + Clone + Send + Sync + 'static,
    {
        let mut root = Dir::new(store.clone());

        // Create app_dir with whiteout and modified file
        let mut app_dir = Dir::new(store.clone());
        let whiteout = File::new(store.clone());
        app_dir
            .put_adapted_file(
                &format!("{}{}", WHITEOUT_PREFIX, "to_be_deleted.txt"),
                whiteout,
            )
            .await?;

        let modified_file1 =
            File::with_content(store.clone(), b"modified file1".as_slice()).await?;
        app_dir
            .put_adapted_file("file1.txt", modified_file1)
            .await?;

        root.put_adapted_dir("app_dir", app_dir).await?;

        // Create whiteout_dir with opaque marker and new file
        let mut whiteout_dir = Dir::new(store.clone());
        let opaque_marker = File::new(store.clone());
        whiteout_dir
            .put_adapted_file(OPAQUE_WHITEOUT_MARKER, opaque_marker)
            .await?;

        let new_file = File::with_content(store.clone(), b"new file in overlay".as_slice()).await?;
        whiteout_dir
            .put_adapted_file("new_file.txt", new_file)
            .await?;

        root.put_adapted_dir("whiteout_dir", whiteout_dir).await?;

        Ok(root)
    }

    /// Creates a top layer with new files and modifications.
    /// Layer structure:
    /// ```text
    /// root_dir/
    /// ├── new_dir/
    /// │   └── new_file.txt ("new file")
    /// └── app_dir/
    ///     └── file2.txt ("modified file2")
    /// ```
    pub async fn create_top_layer<S>(store: S) -> MonocoreResult<Dir<S>>
    where
        S: IpldStore + Clone + Send + Sync + 'static,
    {
        let mut root = Dir::new(store.clone());

        // Create new_dir with new file
        let mut new_dir = Dir::new(store.clone());
        let new_file = File::with_content(store.clone(), b"new file".as_slice()).await?;
        new_dir.put_adapted_file("new_file.txt", new_file).await?;

        root.put_adapted_dir("new_dir", new_dir).await?;

        // Create app_dir with modified file2
        let mut app_dir = Dir::new(store.clone());
        let modified_file2 =
            File::with_content(store.clone(), b"modified file2".as_slice()).await?;
        app_dir
            .put_adapted_file("file2.txt", modified_file2)
            .await?;

        root.put_adapted_dir("app_dir", app_dir).await?;

        Ok(root)
    }

    /// Helper function to verify the contents of a file in a directory
    pub async fn verify_file_content<S>(
        dir: &Dir<S>,
        path: &str,
        expected_content: &str,
    ) -> MonocoreResult<()>
    where
        S: IpldStore + IpldStoreSeekable + Clone + Send + Sync + 'static,
    {
        let parts: Vec<&str> = path.split('/').collect();
        let (dir_path, filename) = parts.split_at(parts.len() - 1);

        // Navigate to the correct directory
        let mut current_dir = dir;
        for dir_name in dir_path {
            current_dir = current_dir
                .get_dir(dir_name)
                .await?
                .expect("Directory not found");
        }

        // Get and verify file content
        let file = current_dir
            .get_file(filename[0])
            .await?
            .expect("File not found");
        let mut content = Vec::new();
        file.get_input_stream()
            .await?
            .read_to_end(&mut content)
            .await
            .map_err(|e| MonocoreError::from(anyhow::Error::from(e)))?;
        String::from_utf8(content)
            .map_err(|e| MonocoreError::from(anyhow::Error::from(e)))
            .map(|s| assert_eq!(s, expected_content))?;
        Ok(())
    }
}

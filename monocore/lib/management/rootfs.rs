//! Root filesystem management for Monocore sandboxes.
//!
//! This module provides functionality for managing root filesystems used by Monocore sandboxes.
//! It handles the creation, extraction, and merging of filesystem layers following OCI (Open
//! Container Initiative) specifications.

use std::{
    borrow::Cow, collections::HashMap, fs::Permissions, os::unix::fs::PermissionsExt, path::Path,
};

use tokio::fs;

use crate::{config::PathPair, vm::VIRTIOFS_TAG_PREFIX, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The opaque directory marker file name used in OCI layers.
pub const OPAQUE_WHITEOUT_MARKER: &str = ".wh..wh..opq";

/// The prefix for whiteout files in OCI layers.
pub const WHITEOUT_PREFIX: &str = ".wh.";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Updates a rootfs by adding sandbox script files to a `/.sandbox_scripts` directory.
///
/// This function:
/// 1. Creates a `.sandbox_scripts` directory under the rootfs if it doesn't exist
/// 2. For each script in the provided HashMap, creates a file with the given name
/// 3. Adds a shebang line using the provided shell path
/// 4. Makes the script files executable (rwxr-x---)
/// 5. Creates a `shell` script containing just the shell path
///
/// ## Arguments
///
/// * `root_path` - Path to the root of the filesystem to patch
/// * `scripts` - HashMap containing script names and their contents
/// * `shell_path` - Path to the shell binary within the rootfs (e.g. "/bin/sh")
pub async fn patch_with_sandbox_scripts(
    scripts_dir: &Path,
    scripts: Cow<'_, HashMap<String, String>>,
    shell_path: impl AsRef<Path>,
) -> MonocoreResult<()> {
    // Remove the scripts directory if it exists
    if scripts_dir.exists() {
        fs::remove_dir_all(&scripts_dir).await?;
    }

    // Create the directory if it doesn't exist
    fs::create_dir_all(&scripts_dir).await?;

    // Get shell path as string for shebang
    let shell_path = shell_path.as_ref().to_string_lossy();
    for (script_name, script_content) in scripts.iter() {
        // Create script file path
        let script_path = scripts_dir.join(script_name);

        // Write shebang and content
        let full_content = format!("#!{}\n{}\n", shell_path, script_content);
        fs::write(&script_path, full_content).await?;

        // Make executable for user and group (rwxr-x---)
        fs::set_permissions(&script_path, Permissions::from_mode(0o750)).await?;
    }

    // Create shell script containing just the shell path
    let shell_script_path = scripts_dir.join("shell");
    fs::write(&shell_script_path, shell_path.to_string()).await?;
    fs::set_permissions(&shell_script_path, Permissions::from_mode(0o750)).await?;

    Ok(())
}

/// Updates the /etc/fstab file in the guest rootfs to mount the mapped directories.
/// Creates the file if it doesn't exist.
///
/// This method:
/// 1. Creates or updates the /etc/fstab file in the guest rootfs
/// 2. Adds entries for each mapped directory using virtio-fs
/// 3. Creates the mount points in the guest rootfs
/// 4. Sets appropriate permissions on the fstab file
///
/// ## Format
/// Each mapped directory is mounted using virtiofs with the following format:
/// ```text
/// virtiofs_N  /guest/path  virtiofs  defaults  0  0
/// ```
/// where N is the index of the mapped directory.
///
/// ## Arguments
/// * `root_path` - Path to the guest rootfs
/// * `mapped_dirs` - List of host:guest directory mappings to mount
///
/// ## Errors
/// Returns an error if:
/// - Cannot create directories in the rootfs
/// - Cannot read or write the fstab file
/// - Cannot set permissions on the fstab file
pub async fn patch_with_virtiofs_mounts(
    root_path: &Path,
    mapped_dirs: &[PathPair],
) -> MonocoreResult<()> {
    let fstab_path = root_path.join("etc/fstab");

    // Create parent directories if they don't exist
    if let Some(parent) = fstab_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Read existing fstab content if it exists
    let mut fstab_content = if fstab_path.exists() {
        fs::read_to_string(&fstab_path).await?
    } else {
        String::new()
    };

    // Add header comment if file is empty
    if fstab_content.is_empty() {
        fstab_content.push_str(
            "# /etc/fstab: static file system information.\n\
                 # <file system>\t<mount point>\t<type>\t<options>\t<dump>\t<pass>\n",
        );
    }

    // Add entries for mapped directories
    for (idx, dir) in mapped_dirs.iter().enumerate() {
        let tag = format!("{}_{}", VIRTIOFS_TAG_PREFIX, idx);
        let guest_path = dir.get_guest();

        // Add entry for this mapped directory
        fstab_content.push_str(&format!(
            "{}\t{}\tvirtiofs\tdefaults\t0\t0\n",
            tag, guest_path
        ));

        // Create the mount point directory in the guest rootfs
        // Convert guest path to a relative path by removing leading slash
        let guest_path_str = guest_path.as_str();
        let relative_path = guest_path_str.strip_prefix('/').unwrap_or(guest_path_str);
        let mount_point = root_path.join(relative_path);
        fs::create_dir_all(mount_point).await?;
    }

    // Write updated fstab content
    fs::write(&fstab_path, fstab_content).await?;

    // Set proper permissions (644 - rw-r--r--)
    let perms = fs::metadata(&fstab_path).await?.permissions();
    let mut new_perms = perms;
    new_perms.set_mode(0o644);
    fs::set_permissions(&fstab_path, new_perms).await?;

    Ok(())
}

/// Updates the /etc/hosts file in the guest rootfs to add hostname mappings.
/// Creates the file if it doesn't exist.
///
/// This method:
/// 1. Creates or updates the /etc/hosts file in the guest rootfs
/// 2. Adds entries for each IP address and hostname pair
/// 3. Sets appropriate permissions on the hosts file
///
/// ## Format
/// Each hostname mapping follows the standard hosts file format:
/// ```text
/// 192.168.1.100  hostname1
/// 192.168.1.101  hostname2
/// ```
///
/// ## Arguments
/// * `root_path` - Path to the guest rootfs
/// * `hostname_mappings` - List of (IPv4 address, hostname) pairs to add
///
/// ## Errors
/// Returns an error if:
/// - Cannot create directories in the rootfs
/// - Cannot read or write the hosts file
/// - Cannot set permissions on the hosts file
async fn _patch_with_hostnames(
    root_path: &Path,
    hostname_mappings: &[(std::net::Ipv4Addr, String)],
) -> MonocoreResult<()> {
    let hosts_path = root_path.join("etc/hosts");

    // Create parent directories if they don't exist
    if let Some(parent) = hosts_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Read existing hosts content if it exists
    let mut hosts_content = if hosts_path.exists() {
        fs::read_to_string(&hosts_path).await?
    } else {
        String::new()
    };

    // Add header comment if file is empty
    if hosts_content.is_empty() {
        hosts_content.push_str(
            "# /etc/hosts: static table lookup for hostnames.\n\
             # <ip-address>\t<hostname>\n\n\
             127.0.0.1\tlocalhost\n\
             ::1\tlocalhost ip6-localhost ip6-loopback\n",
        );
    }

    // Add entries for hostname mappings
    for (ip_addr, hostname) in hostname_mappings {
        // Check if this mapping already exists
        let entry = format!("{}\t{}", ip_addr, hostname);
        if !hosts_content.contains(&entry) {
            hosts_content.push_str(&format!("{}\n", entry));
        }
    }

    // Write updated hosts content
    fs::write(&hosts_path, hosts_content).await?;

    // Set proper permissions (644 - rw-r--r--)
    let perms = fs::metadata(&hosts_path).await?.permissions();
    let mut new_perms = perms;
    new_perms.set_mode(0o644);
    fs::set_permissions(&hosts_path, new_perms).await?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::MonocoreError;

    use super::*;

    #[tokio::test]
    async fn test_patch_rootfs_with_virtiofs_mounts() -> anyhow::Result<()> {
        // Create a temporary directory to act as our rootfs
        let root_dir = TempDir::new()?;
        let root_path = root_dir.path();

        // Create temporary directories for host paths
        let host_dir = TempDir::new()?;
        let host_data = host_dir.path().join("data");
        let host_config = host_dir.path().join("config");
        let host_app = host_dir.path().join("app");

        // Create the host directories
        fs::create_dir_all(&host_data).await?;
        fs::create_dir_all(&host_config).await?;
        fs::create_dir_all(&host_app).await?;

        // Create test directory mappings using our temporary paths
        let mapped_dirs = vec![
            format!("{}:/container/data", host_data.display()).parse::<PathPair>()?,
            format!("{}:/etc/app/config", host_config.display()).parse::<PathPair>()?,
            format!("{}:/app", host_app.display()).parse::<PathPair>()?,
        ];

        // Update fstab
        patch_with_virtiofs_mounts(root_path, &mapped_dirs).await?;

        // Verify fstab file was created with correct content
        let fstab_path = root_path.join("etc/fstab");
        assert!(fstab_path.exists());

        let fstab_content = fs::read_to_string(&fstab_path).await?;

        // Check header
        assert!(fstab_content.contains("# /etc/fstab: static file system information"));
        assert!(fstab_content
            .contains("<file system>\t<mount point>\t<type>\t<options>\t<dump>\t<pass>"));

        // Check entries
        assert!(fstab_content.contains("virtiofs_0\t/container/data\tvirtiofs\tdefaults\t0\t0"));
        assert!(fstab_content.contains("virtiofs_1\t/etc/app/config\tvirtiofs\tdefaults\t0\t0"));
        assert!(fstab_content.contains("virtiofs_2\t/app\tvirtiofs\tdefaults\t0\t0"));

        // Verify mount points were created
        assert!(root_path.join("container/data").exists());
        assert!(root_path.join("etc/app/config").exists());
        assert!(root_path.join("app").exists());

        // Verify file permissions
        let perms = fs::metadata(&fstab_path).await?.permissions();
        assert_eq!(perms.mode() & 0o777, 0o644);

        // Test updating existing fstab
        let host_logs = host_dir.path().join("logs");
        fs::create_dir_all(&host_logs).await?;

        let new_mapped_dirs = vec![
            format!("{}:/container/data", host_data.display()).parse::<PathPair>()?, // Keep one existing
            format!("{}:/var/log", host_logs.display()).parse::<PathPair>()?,        // Add new one
        ];

        // Update fstab again
        patch_with_virtiofs_mounts(root_path, &new_mapped_dirs).await?;

        // Verify updated content
        let updated_content = fs::read_to_string(&fstab_path).await?;
        assert!(updated_content.contains("virtiofs_0\t/container/data\tvirtiofs\tdefaults\t0\t0"));
        assert!(updated_content.contains("virtiofs_1\t/var/log\tvirtiofs\tdefaults\t0\t0"));

        // Verify new mount point was created
        assert!(root_path.join("var/log").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_patch_rootfs_with_virtiofs_mounts_permission_errors() -> anyhow::Result<()> {
        // Skip this test in CI environments
        if std::env::var("CI").is_ok() {
            println!("Skipping permission test in CI environment");
            return Ok(());
        }

        // Setup a rootfs where we can't write the fstab file
        let readonly_dir = TempDir::new()?;
        let readonly_path = readonly_dir.path();
        let etc_path = readonly_path.join("etc");
        fs::create_dir_all(&etc_path).await?;

        // Make /etc directory read-only to simulate permission issues
        let mut perms = fs::metadata(&etc_path).await?.permissions();
        perms.set_mode(0o400); // read-only
        fs::set_permissions(&etc_path, perms).await?;

        // Verify permissions were actually set (helpful for debugging)
        let actual_perms = fs::metadata(&etc_path).await?.permissions();
        println!("Set /etc permissions to: {:o}", actual_perms.mode());

        // Try to update fstab in a read-only /etc directory
        let host_dir = TempDir::new()?;
        let host_path = host_dir.path().join("test");
        fs::create_dir_all(&host_path).await?;

        let mapped_dirs =
            vec![format!("{}:/container/data", host_path.display()).parse::<PathPair>()?];

        // Function should detect it cannot write to /etc/fstab and return an error
        let result = patch_with_virtiofs_mounts(readonly_path, &mapped_dirs).await;

        // Detailed error reporting for debugging
        if result.is_ok() {
            println!("Warning: Write succeeded despite read-only permissions");
            println!(
                "Current /etc permissions: {:o}",
                fs::metadata(&etc_path).await?.permissions().mode()
            );
            if etc_path.join("fstab").exists() {
                println!(
                    "fstab file was created with permissions: {:o}",
                    fs::metadata(etc_path.join("fstab"))
                        .await?
                        .permissions()
                        .mode()
                );
            }
        }

        assert!(
            result.is_err(),
            "Expected error when writing fstab to read-only /etc directory. \
             Current /etc permissions: {:o}",
            fs::metadata(&etc_path).await?.permissions().mode()
        );
        assert!(matches!(result.unwrap_err(), MonocoreError::Io(_)));

        Ok(())
    }

    #[tokio::test]
    async fn test_patch_with_hostnames() -> anyhow::Result<()> {
        use std::net::Ipv4Addr;

        // Create a temporary directory to act as our rootfs
        let root_dir = TempDir::new()?;
        let root_path = root_dir.path();

        // Create test hostname mappings
        let hostname_mappings = vec![
            (Ipv4Addr::new(192, 168, 1, 100), "host1.local".to_string()),
            (Ipv4Addr::new(192, 168, 1, 101), "host2.local".to_string()),
        ];

        // Update hosts file
        _patch_with_hostnames(root_path, &hostname_mappings).await?;

        // Verify hosts file was created with correct content
        let hosts_path = root_path.join("etc/hosts");
        assert!(hosts_path.exists());

        let hosts_content = fs::read_to_string(&hosts_path).await?;

        // Check header
        assert!(hosts_content.contains("# /etc/hosts: static table lookup for hostnames"));
        assert!(hosts_content.contains("127.0.0.1\tlocalhost"));
        assert!(hosts_content.contains("::1\tlocalhost ip6-localhost ip6-loopback"));

        // Check entries
        assert!(hosts_content.contains("192.168.1.100\thost1.local"));
        assert!(hosts_content.contains("192.168.1.101\thost2.local"));

        // Verify file permissions
        let perms = fs::metadata(&hosts_path).await?.permissions();
        assert_eq!(perms.mode() & 0o777, 0o644);

        // Test updating existing hosts file with new entries
        let new_mappings = vec![
            (Ipv4Addr::new(192, 168, 1, 100), "host1.local".to_string()), // Existing entry
            (Ipv4Addr::new(192, 168, 1, 102), "host3.local".to_string()), // New entry
        ];

        // Update hosts file again
        _patch_with_hostnames(root_path, &new_mappings).await?;

        // Verify updated content
        let updated_content = fs::read_to_string(&hosts_path).await?;

        // Should still contain original entries
        assert!(updated_content.contains("127.0.0.1\tlocalhost"));
        assert!(updated_content.contains("::1\tlocalhost ip6-localhost ip6-loopback"));

        // Should contain both old and new entries without duplicates
        assert!(updated_content.contains("192.168.1.100\thost1.local"));
        assert!(updated_content.contains("192.168.1.102\thost3.local"));

        // Count occurrences of the first IP to ensure no duplicates
        let count = updated_content
            .lines()
            .filter(|line| line.contains("192.168.1.100"))
            .count();
        assert_eq!(count, 1, "Should not have duplicate entries");

        Ok(())
    }
}

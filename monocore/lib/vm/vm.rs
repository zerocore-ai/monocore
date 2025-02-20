use std::{
    ffi::CString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use getset::Getters;
use monoutils::SupportedPathType;
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{EnvPair, PathPair, PortPair},
    utils, InvalidMicroVMConfigError, MonocoreError, MonocoreResult,
};

use super::{ffi, LinuxRlimit, MicroVmBuilder, MicroVmConfigBuilder};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The prefix used for virtio-fs tags when mounting shared directories
const VIRTIOFS_TAG_PREFIX: &str = "virtiofs";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A lightweight Linux virtual machine.
///
/// MicroVm provides a secure, isolated environment for running applications with their own
/// filesystem, network, and resource constraints.
///
/// ## Examples
///
/// ```rust
/// use monocore::vm::MicroVm;
/// use tempfile::TempDir;
///
/// # fn main() -> anyhow::Result<()> {
/// let temp_dir = TempDir::new()?;
/// let vm = MicroVm::builder()
///     .root_path(temp_dir.path())
///     .ram_mib(1024)
///     .exec_path("/bin/echo")
///     .args(["Hello, World!"])
///     .build()?;
///
/// // // Start the MicroVm
/// // vm.start()?;  // This would actually run the VM
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Getters)]
pub struct MicroVm {
    /// The context ID for the MicroVm.
    ctx_id: u32,

    /// The configuration for the MicroVm.
    #[get = "pub with_prefix"]
    config: MicroVmConfig,
}

/// Configuration for a MicroVm instance.
///
/// This struct holds all the settings needed to create and run a MicroVm,
/// including system resources, filesystem configuration, networking, and
/// process execution details.
///
/// Rather than creating this directly, use `MicroVmConfigBuilder` or
/// `MicroVmBuilder` for a more ergonomic interface.
///
/// ## Examples
///
/// ```rust
/// use monocore::vm::{MicroVm, MicroVmConfig};
/// use tempfile::TempDir;
///
/// # fn main() -> anyhow::Result<()> {
/// let temp_dir = TempDir::new()?;
/// let config = MicroVmConfig::builder()
///     .root_path(temp_dir.path())
///     .ram_mib(1024)
///     .build();
///
/// let vm = MicroVm::from_config(config)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct MicroVmConfig {
    /// The log level to use for the MicroVm.
    pub log_level: LogLevel,

    /// The path to the root directory for the MicroVm.
    pub root_path: PathBuf,

    /// The number of vCPUs to use for the MicroVm.
    pub num_vcpus: u8,

    /// The amount of RAM in MiB to use for the MicroVm.
    pub ram_mib: u32,

    /// The directories to mount in the MicroVm using virtio-fs.
    /// Each PathPair represents a host:guest path mapping.
    pub mapped_dirs: Vec<PathPair>,

    /// The port map to use for the MicroVm.
    pub port_map: Vec<PortPair>,

    /// The resource limits to use for the MicroVm.
    pub rlimits: Vec<LinuxRlimit>,

    /// The working directory path to use for the MicroVm.
    pub workdir_path: Option<Utf8UnixPathBuf>,

    /// The executable path to use for the MicroVm.
    pub exec_path: Option<Utf8UnixPathBuf>,

    /// The arguments to pass to the executable.
    pub args: Vec<String>,

    /// The environment variables to set for the executable.
    pub env: Vec<EnvPair>,

    /// The console output path to use for the MicroVm.
    pub console_output: Option<Utf8UnixPathBuf>,
}

/// The log level to use for the MicroVm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LogLevel {
    /// No logging.
    #[default]
    Off = 0,

    /// Error messages.
    Error = 1,

    /// Warning messages.
    Warn = 2,

    /// Informational messages.
    Info = 3,

    /// Debug messages.
    Debug = 4,

    /// Trace messages.
    Trace = 5,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MicroVm {
    /// Creates a new MicroVm from the given configuration.
    ///
    /// This is a low-level constructor - prefer using `MicroVm::builder()`
    /// for a more ergonomic interface.
    ///
    /// ## Errors
    /// Returns an error if:
    /// - The configuration is invalid
    /// - Required resources cannot be allocated
    /// - The system lacks required capabilities
    pub fn from_config(config: MicroVmConfig) -> MonocoreResult<Self> {
        let ctx_id = Self::create_ctx();

        config.validate()?;

        Self::apply_config(ctx_id, &config);

        Ok(Self { ctx_id, config })
    }

    /// Creates a builder for configuring a new MicroVm instance.
    ///
    /// This is the recommended way to create a new MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVm;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVm::builder()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> MicroVmBuilder<(), ()> {
        MicroVmBuilder::default()
    }

    /// Starts the MicroVm and waits for it to complete.
    ///
    /// This function will block until the MicroVm exits. The exit status
    /// of the guest process is returned.
    ///
    /// ## Examples
    ///
    /// ```rust,no_run
    /// use monocore::vm::MicroVm;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVm::builder()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .exec_path("/usr/bin/python3")
    ///     .args(["-c", "print('Hello from MicroVm!')"])
    ///     .build()?;
    ///
    /// // let status = vm.start()?;
    /// // assert_eq!(status, 0);  // Process exited successfully
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - This function takes control of stdin/stdout
    /// - The MicroVm is automatically cleaned up when this returns
    /// - A non-zero status indicates the guest process failed
    pub fn start(&self) -> MonocoreResult<i32> {
        let ctx_id = self.ctx_id;
        let status = unsafe { ffi::krun_start_enter(ctx_id) };
        if status < 0 {
            tracing::error!("failed to start microvm: {}", status);
            return Err(MonocoreError::StartVmFailed(status));
        }
        tracing::info!("microvm exited with status: {}", status);
        Ok(status)
    }

    fn create_ctx() -> u32 {
        let ctx_id = unsafe { ffi::krun_create_ctx() };
        assert!(ctx_id >= 0, "failed to create microvm context: {}", ctx_id);
        ctx_id as u32
    }

    // TODO: Rename to patch_rootfs_fstab_with_virtiofs_mounts and move to management::rootfs.
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
    fn update_rootfs_fstab(root_path: &Path, mapped_dirs: &[PathPair]) -> MonocoreResult<()> {
        let fstab_path = root_path.join("etc/fstab");

        // Create parent directories if they don't exist
        if let Some(parent) = fstab_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read existing fstab content if it exists
        let mut fstab_content = if fstab_path.exists() {
            fs::read_to_string(&fstab_path)?
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
            fs::create_dir_all(mount_point)?;
        }

        // Write updated fstab content
        fs::write(&fstab_path, fstab_content)?;

        // Set proper permissions (644 - rw-r--r--)
        let perms = fs::metadata(&fstab_path)?.permissions();
        let mut new_perms = perms;
        new_perms.set_mode(0o644);
        fs::set_permissions(&fstab_path, new_perms)?;

        Ok(())
    }

    /// Applies the configuration to the MicroVm context.
    ///
    /// This method configures all aspects of the MicroVm including:
    /// - Basic VM settings (vCPUs, RAM)
    /// - Root filesystem
    /// - Directory mappings via virtio-fs
    /// - Port mappings
    /// - Resource limits
    /// - Working directory
    /// - Executable and arguments
    /// - Environment variables
    /// - Console output
    /// - Network settings
    ///
    /// ## Arguments
    /// * `ctx_id` - The MicroVm context ID to configure
    /// * `config` - The configuration to apply
    ///
    /// ## Panics
    /// Panics if:
    /// - Any libkrun API call fails
    /// - Cannot update the rootfs fstab file
    fn apply_config(ctx_id: u32, config: &MicroVmConfig) {
        // Set log level
        unsafe {
            let status = ffi::krun_set_log_level(config.log_level as u32);
            assert!(status >= 0, "failed to set log level: {}", status);
        }

        // Set basic VM configuration
        unsafe {
            let status = ffi::krun_set_vm_config(ctx_id, config.num_vcpus, config.ram_mib);
            assert!(status >= 0, "failed to set VM config: {}", status);
        }

        // Set root path
        let c_root_path = CString::new(config.root_path.to_str().unwrap().as_bytes()).unwrap();
        unsafe {
            let status = ffi::krun_set_root(ctx_id, c_root_path.as_ptr());
            assert!(status >= 0, "failed to set root path: {}", status);
        }

        // Add mapped directories using virtio-fs
        // First, update the rootfs fstab to mount the directories
        let root_path = &config.root_path;
        let mapped_dirs = &config.mapped_dirs;

        // TODO: Don't do update here
        // Update fstab
        if let Err(e) = Self::update_rootfs_fstab(root_path, mapped_dirs) {
            tracing::error!("failed to update rootfs fstab: {}", e);
            panic!("failed to update rootfs fstab: {}", e);
        }

        // Then add the virtiofs mounts
        for (idx, dir) in mapped_dirs.iter().enumerate() {
            let tag = CString::new(format!("{}_{}", VIRTIOFS_TAG_PREFIX, idx)).unwrap();
            let host_path = CString::new(dir.get_host().to_string().as_bytes()).unwrap();
            unsafe {
                let status = ffi::krun_add_virtiofs(ctx_id, tag.as_ptr(), host_path.as_ptr());
                assert!(status >= 0, "failed to add mapped directory: {}", status);
            }
        }

        // Set port map
        let c_port_map: Vec<_> = config
            .port_map
            .iter()
            .map(|p| CString::new(p.to_string()).unwrap())
            .collect();
        let c_port_map_ptrs = utils::to_null_terminated_c_array(&c_port_map);

        unsafe {
            let status = ffi::krun_set_port_map(ctx_id, c_port_map_ptrs.as_ptr());
            assert!(status >= 0, "failed to set port map: {}", status);
        }

        // Set resource limits
        if !config.rlimits.is_empty() {
            let c_rlimits: Vec<_> = config
                .rlimits
                .iter()
                .map(|s| CString::new(s.to_string()).unwrap())
                .collect();
            let c_rlimits_ptrs = utils::to_null_terminated_c_array(&c_rlimits);
            unsafe {
                let status = ffi::krun_set_rlimits(ctx_id, c_rlimits_ptrs.as_ptr());
                assert!(status >= 0, "failed to set resource limits: {}", status);
            }
        }

        // Set working directory
        if let Some(workdir) = &config.workdir_path {
            let c_workdir = CString::new(workdir.to_string().as_bytes()).unwrap();
            unsafe {
                let status = ffi::krun_set_workdir(ctx_id, c_workdir.as_ptr());
                assert!(status >= 0, "Failed to set working directory: {}", status);
            }
        }

        // Set executable path, arguments, and environment variables
        if let Some(exec_path) = &config.exec_path {
            let c_exec_path = CString::new(exec_path.to_string().as_bytes()).unwrap();

            let c_argv: Vec<_> = config
                .args
                .iter()
                .map(|s| CString::new(s.as_str()).unwrap())
                .collect();
            let c_argv_ptrs = utils::to_null_terminated_c_array(&c_argv);

            let c_env: Vec<_> = config
                .env
                .iter()
                .map(|s| CString::new(s.to_string()).unwrap())
                .collect();
            let c_env_ptrs = utils::to_null_terminated_c_array(&c_env);

            unsafe {
                let status = ffi::krun_set_exec(
                    ctx_id,
                    c_exec_path.as_ptr(),
                    c_argv_ptrs.as_ptr(),
                    c_env_ptrs.as_ptr(),
                );
                assert!(
                    status >= 0,
                    "Failed to set executable configuration: {}",
                    status
                );
            }
        } else {
            // If no executable path is set, we still need to set the environment variables
            let c_env: Vec<_> = config
                .env
                .iter()
                .map(|s| CString::new(s.to_string()).unwrap())
                .collect();
            let c_env_ptrs = utils::to_null_terminated_c_array(&c_env);

            unsafe {
                let status = ffi::krun_set_env(ctx_id, c_env_ptrs.as_ptr());
                assert!(
                    status >= 0,
                    "Failed to set environment variables: {}",
                    status
                );
            }
        }

        // Set console output
        if let Some(console_output) = &config.console_output {
            let c_console_output = CString::new(console_output.to_string().as_bytes()).unwrap();
            unsafe {
                let status = ffi::krun_set_console_output(ctx_id, c_console_output.as_ptr());
                assert!(status >= 0, "Failed to set console output: {}", status);
            }
        }
    }
}

impl MicroVmConfig {
    /// Creates a builder for configuring a new MicroVm configuration.
    ///
    /// This is the recommended way to create a new MicroVmConfig instance. The builder pattern
    /// provides a more ergonomic interface and ensures all required fields are set.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfig;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let config = MicroVmConfig::builder()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> MicroVmConfigBuilder<(), ()> {
        MicroVmConfigBuilder::default()
    }

    /// Validates that guest paths are not subsets of each other.
    ///
    /// For example, these paths would conflict:
    /// - /app and /app/data
    /// - /var/log and /var
    /// - /data and /data
    ///
    /// ## Arguments
    /// * `mapped_dirs` - The mapped directories to validate
    ///
    /// ## Returns
    /// - Ok(()) if no paths are subsets of each other
    /// - Err with details about conflicting paths
    fn validate_guest_paths(mapped_dirs: &[PathPair]) -> MonocoreResult<()> {
        // Early return if we have 0 or 1 paths - no conflicts possible
        if mapped_dirs.len() <= 1 {
            return Ok(());
        }

        // Pre-normalize all paths once to avoid repeated normalization
        let normalized_paths: Vec<_> = mapped_dirs
            .iter()
            .map(|dir| {
                monoutils::normalize_path(dir.get_guest().as_str(), SupportedPathType::Absolute)
                    .map_err(Into::into)
            })
            .collect::<MonocoreResult<Vec<_>>>()?;

        // Compare each path with every other path only once
        // Using windows of size 2 would miss some comparisons since we need to check both directions
        for i in 0..normalized_paths.len() {
            let path1 = &normalized_paths[i];

            // Only need to check paths after this one since previous comparisons were already done
            for path2 in &normalized_paths[i + 1..] {
                // Check both directions for prefix relationship
                if utils::paths_overlap(path1, path2) {
                    return Err(MonocoreError::InvalidMicroVMConfig(
                        InvalidMicroVMConfigError::ConflictingGuestPaths(
                            path1.to_string(),
                            path2.to_string(),
                        ),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validates the MicroVm configuration.
    ///
    /// Performs a series of checks to ensure the configuration is valid:
    /// - Verifies the root path exists and is accessible
    /// - Verifies all host paths in mapped_dirs exist and are accessible
    /// - Ensures number of vCPUs is non-zero
    /// - Ensures RAM allocation is non-zero
    /// - Validates executable path and arguments contain only printable ASCII characters
    /// - Validates guest paths don't overlap or conflict with each other
    ///
    /// ## Returns
    /// - `Ok(())` if the configuration is valid
    /// - `Err(MonocoreError::InvalidMicroVMConfig)` with details about what failed
    ///
    /// ## Examples
    /// ```rust
    /// use monocore::vm::MicroVmConfig;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let config = MicroVmConfig::builder()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .build();
    ///
    /// assert!(config.validate().is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> MonocoreResult<()> {
        // Check root path exists
        if !self.root_path.exists() {
            return Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::RootPathDoesNotExist(
                    self.root_path.to_str().unwrap().into(),
                ),
            ));
        }

        // Check all host paths in mapped_dirs exist
        for dir in &self.mapped_dirs {
            let host_path = PathBuf::from(dir.get_host().as_str());
            if !host_path.exists() {
                return Err(MonocoreError::InvalidMicroVMConfig(
                    InvalidMicroVMConfigError::HostPathDoesNotExist(
                        host_path.to_str().unwrap().into(),
                    ),
                ));
            }
        }

        if self.num_vcpus == 0 {
            return Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::NumVCPUsIsZero,
            ));
        }

        if self.ram_mib == 0 {
            return Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::RamIsZero,
            ));
        }

        if let Some(exec_path) = &self.exec_path {
            Self::validate_command_line(exec_path.as_ref())?;
        }

        for arg in &self.args {
            Self::validate_command_line(arg)?;
        }

        // Validate guest paths are not subsets of each other
        Self::validate_guest_paths(&self.mapped_dirs)?;

        Ok(())
    }

    /// Validates that a command line string contains only allowed characters.
    ///
    /// Command line strings (executable paths and arguments) must contain only printable ASCII
    /// characters in the range from space (0x20) to tilde (0x7E). This excludes:
    /// - Control characters (newlines, tabs, etc.)
    /// - Non-ASCII Unicode characters
    /// - Null bytes
    ///
    /// ## Arguments
    /// * `s` - The string to validate
    ///
    /// ## Returns
    /// - `Ok(())` if the string contains only valid characters
    /// - `Err(MonocoreError::InvalidMicroVMConfig)` if invalid characters are found
    ///
    /// ## Examples
    /// ```rust
    /// use monocore::vm::MicroVmConfig;
    ///
    /// // Valid strings
    /// assert!(MicroVmConfig::validate_command_line("/bin/echo").is_ok());
    /// assert!(MicroVmConfig::validate_command_line("Hello, World!").is_ok());
    ///
    /// // Invalid strings
    /// assert!(MicroVmConfig::validate_command_line("/bin/echo\n").is_err());  // newline
    /// assert!(MicroVmConfig::validate_command_line("hello🌎").is_err());      // emoji
    /// ```
    pub fn validate_command_line(s: &str) -> MonocoreResult<()> {
        fn valid_char(c: char) -> bool {
            matches!(c, ' '..='~')
        }

        if s.chars().all(valid_char) {
            Ok(())
        } else {
            Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::InvalidCommandLineString(s.to_string()),
            ))
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Drop for MicroVm {
    fn drop(&mut self) {
        unsafe { ffi::krun_free_ctx(self.ctx_id) };
    }
}

impl TryFrom<u8> for LogLevel {
    type Error = MonocoreError;

    fn try_from(value: u8) -> Result<Self, MonocoreError> {
        match value {
            0 => Ok(LogLevel::Off),
            1 => Ok(LogLevel::Error),
            2 => Ok(LogLevel::Warn),
            3 => Ok(LogLevel::Info),
            4 => Ok(LogLevel::Debug),
            5 => Ok(LogLevel::Trace),
            _ => Err(MonocoreError::InvalidLogLevel(value)),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::config::DEFAULT_NUM_VCPUS;

    use super::*;
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    use tempfile::TempDir;

    #[test]
    fn test_microvm_config_builder() {
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .root_path(PathBuf::from("/tmp"))
            .ram_mib(512)
            .build();

        assert!(config.log_level == LogLevel::Info);
        assert_eq!(config.root_path, PathBuf::from("/tmp"));
        assert_eq!(config.ram_mib, 512);
        assert_eq!(config.num_vcpus, DEFAULT_NUM_VCPUS);
    }

    #[test]
    fn test_microvm_config_validation_success() {
        let temp_dir = TempDir::new().unwrap();
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .root_path(temp_dir.path().to_path_buf())
            .ram_mib(512)
            .build();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_microvm_config_validation_failure_root_path() {
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .root_path(PathBuf::from("/non/existent/path"))
            .ram_mib(512)
            .build();

        assert!(matches!(
            config.validate(),
            Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::RootPathDoesNotExist(_)
            ))
        ));
    }

    #[test]
    fn test_microvm_config_validation_failure_zero_ram() {
        let temp_dir = TempDir::new().unwrap();
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .root_path(temp_dir.path().to_path_buf())
            .ram_mib(0)
            .build();

        assert!(matches!(
            config.validate(),
            Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::RamIsZero
            ))
        ));
    }

    #[test]
    fn test_validate_command_line_valid_strings() {
        // Test basic ASCII strings
        assert!(MicroVmConfig::validate_command_line("hello").is_ok());
        assert!(MicroVmConfig::validate_command_line("hello world").is_ok());
        assert!(MicroVmConfig::validate_command_line("Hello, World!").is_ok());

        // Test edge cases of valid range (space to tilde)
        assert!(MicroVmConfig::validate_command_line(" ").is_ok()); // space (0x20)
        assert!(MicroVmConfig::validate_command_line("~").is_ok()); // tilde (0x7E)

        // Test special characters within valid range
        assert!(MicroVmConfig::validate_command_line("!@#$%^&*()").is_ok());
        assert!(MicroVmConfig::validate_command_line("path/to/file").is_ok());
        assert!(MicroVmConfig::validate_command_line("user-name_123").is_ok());
    }

    #[test]
    fn test_validate_command_line_invalid_strings() {
        // Test control characters
        assert!(MicroVmConfig::validate_command_line("\n").is_err()); // newline
        assert!(MicroVmConfig::validate_command_line("\t").is_err()); // tab
        assert!(MicroVmConfig::validate_command_line("\r").is_err()); // carriage return
        assert!(MicroVmConfig::validate_command_line("\x1B").is_err()); // escape

        // Test non-ASCII Unicode characters
        assert!(MicroVmConfig::validate_command_line("hello🌎").is_err()); // emoji
        assert!(MicroVmConfig::validate_command_line("über").is_err()); // umlaut
        assert!(MicroVmConfig::validate_command_line("café").is_err()); // accent
        assert!(MicroVmConfig::validate_command_line("你好").is_err()); // Chinese characters

        // Test strings mixing valid and invalid characters
        assert!(MicroVmConfig::validate_command_line("hello\nworld").is_err());
        assert!(MicroVmConfig::validate_command_line("path/to/file\0").is_err()); // null byte
        assert!(MicroVmConfig::validate_command_line("hello\x7F").is_err()); // DEL character
    }

    #[test]
    fn test_validate_command_line_in_config() {
        let temp_dir = TempDir::new().unwrap();

        // Test invalid executable path
        let config = MicroVmConfig::builder()
            .root_path(temp_dir.path().to_path_buf())
            .ram_mib(512)
            .exec_path("/bin/hello\nworld")
            .build();
        assert!(matches!(
            config.validate(),
            Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::InvalidCommandLineString(_)
            ))
        ));

        // Test invalid argument
        let config = MicroVmConfig::builder()
            .root_path(temp_dir.path().to_path_buf())
            .ram_mib(512)
            .exec_path("/bin/echo")
            .args(["hello\tworld"])
            .build();
        assert!(matches!(
            config.validate(),
            Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::InvalidCommandLineString(_)
            ))
        ));
    }

    #[test]
    fn test_update_rootfs_fstab() -> anyhow::Result<()> {
        // Create a temporary directory to act as our rootfs
        let root_dir = TempDir::new()?;
        let root_path = root_dir.path();

        // Create temporary directories for host paths
        let host_dir = TempDir::new()?;
        let host_data = host_dir.path().join("data");
        let host_config = host_dir.path().join("config");
        let host_app = host_dir.path().join("app");

        // Create the host directories
        fs::create_dir_all(&host_data)?;
        fs::create_dir_all(&host_config)?;
        fs::create_dir_all(&host_app)?;

        // Create test directory mappings using our temporary paths
        let mapped_dirs = vec![
            format!("{}:/container/data", host_data.display()).parse::<PathPair>()?,
            format!("{}:/etc/app/config", host_config.display()).parse::<PathPair>()?,
            format!("{}:/app", host_app.display()).parse::<PathPair>()?,
        ];

        // Update fstab
        MicroVm::update_rootfs_fstab(root_path, &mapped_dirs)?;

        // Verify fstab file was created with correct content
        let fstab_path = root_path.join("etc/fstab");
        assert!(fstab_path.exists());

        let fstab_content = fs::read_to_string(&fstab_path)?;

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
        let perms = fs::metadata(&fstab_path)?.permissions();
        assert_eq!(perms.mode() & 0o777, 0o644);

        // Test updating existing fstab
        let host_logs = host_dir.path().join("logs");
        fs::create_dir_all(&host_logs)?;

        let new_mapped_dirs = vec![
            format!("{}:/container/data", host_data.display()).parse::<PathPair>()?, // Keep one existing
            format!("{}:/var/log", host_logs.display()).parse::<PathPair>()?,        // Add new one
        ];

        // Update fstab again
        MicroVm::update_rootfs_fstab(root_path, &new_mapped_dirs)?;

        // Verify updated content
        let updated_content = fs::read_to_string(&fstab_path)?;
        assert!(updated_content.contains("virtiofs_0\t/container/data\tvirtiofs\tdefaults\t0\t0"));
        assert!(updated_content.contains("virtiofs_1\t/var/log\tvirtiofs\tdefaults\t0\t0"));

        // Verify new mount point was created
        assert!(root_path.join("var/log").exists());

        Ok(())
    }

    #[test]
    fn test_update_rootfs_fstab_permission_errors() -> anyhow::Result<()> {
        // Skip this test in CI environments
        if std::env::var("CI").is_ok() {
            println!("Skipping permission test in CI environment");
            return Ok(());
        }

        // Setup a rootfs where we can't write the fstab file
        let readonly_dir = TempDir::new()?;
        let readonly_path = readonly_dir.path();
        let etc_path = readonly_path.join("etc");
        fs::create_dir_all(&etc_path)?;

        // Make /etc directory read-only to simulate permission issues
        let mut perms = fs::metadata(&etc_path)?.permissions();
        perms.set_mode(0o400); // read-only
        fs::set_permissions(&etc_path, perms)?;

        // Verify permissions were actually set (helpful for debugging)
        let actual_perms = fs::metadata(&etc_path)?.permissions();
        println!("Set /etc permissions to: {:o}", actual_perms.mode());

        // Try to update fstab in a read-only /etc directory
        let host_dir = TempDir::new()?;
        let host_path = host_dir.path().join("test");
        fs::create_dir_all(&host_path)?;

        let mapped_dirs =
            vec![format!("{}:/container/data", host_path.display()).parse::<PathPair>()?];

        // Function should detect it cannot write to /etc/fstab and return an error
        let result = MicroVm::update_rootfs_fstab(readonly_path, &mapped_dirs);

        // Detailed error reporting for debugging
        if result.is_ok() {
            println!("Warning: Write succeeded despite read-only permissions");
            println!(
                "Current /etc permissions: {:o}",
                fs::metadata(&etc_path)?.permissions().mode()
            );
            if etc_path.join("fstab").exists() {
                println!(
                    "fstab file was created with permissions: {:o}",
                    fs::metadata(etc_path.join("fstab"))?.permissions().mode()
                );
            }
        }

        assert!(
            result.is_err(),
            "Expected error when writing fstab to read-only /etc directory. \
             Current /etc permissions: {:o}",
            fs::metadata(&etc_path)?.permissions().mode()
        );
        assert!(matches!(result.unwrap_err(), MonocoreError::Io(_)));

        Ok(())
    }

    #[test]
    fn test_validate_guest_paths() -> anyhow::Result<()> {
        // Test valid paths (no conflicts)
        let valid_paths = vec![
            "/app".parse::<PathPair>()?,
            "/data".parse()?,
            "/var/log".parse()?,
            "/etc/config".parse()?,
        ];
        assert!(MicroVmConfig::validate_guest_paths(&valid_paths).is_ok());

        // Test conflicting paths (direct match)
        let conflicting_paths = vec![
            "/app".parse()?,
            "/data".parse()?,
            "/app".parse()?, // Duplicate
        ];
        assert!(MicroVmConfig::validate_guest_paths(&conflicting_paths).is_err());

        // Test conflicting paths (subset)
        let subset_paths = vec![
            "/app".parse()?,
            "/app/data".parse()?, // Subset of /app
            "/var/log".parse()?,
        ];
        assert!(MicroVmConfig::validate_guest_paths(&subset_paths).is_err());

        // Test conflicting paths (parent)
        let parent_paths = vec![
            "/var/log".parse()?,
            "/var".parse()?, // Parent of /var/log
            "/etc".parse()?,
        ];
        assert!(MicroVmConfig::validate_guest_paths(&parent_paths).is_err());

        // Test paths needing normalization
        let unnormalized_paths = vec![
            "/app/./data".parse()?,
            "/var/log".parse()?,
            "/etc//config".parse()?,
        ];
        assert!(MicroVmConfig::validate_guest_paths(&unnormalized_paths).is_ok());

        // Test paths with normalization conflicts
        let normalized_conflicts = vec![
            "/app/./data".parse()?,
            "/app/data/".parse()?, // Same as first path after normalization
            "/var/log".parse()?,
        ];
        assert!(MicroVmConfig::validate_guest_paths(&normalized_conflicts).is_err());

        Ok(())
    }

    #[test]
    fn test_microvm_config_validation_with_guest_paths() -> anyhow::Result<()> {
        use tempfile::TempDir;

        let temp_dir = TempDir::new()?;
        let host_dir1 = temp_dir.path().join("dir1");
        let host_dir2 = temp_dir.path().join("dir2");
        std::fs::create_dir_all(&host_dir1)?;
        std::fs::create_dir_all(&host_dir2)?;

        // Test valid configuration
        let valid_config = MicroVmConfig::builder()
            .root_path(temp_dir.path())
            .ram_mib(1024)
            .mapped_dirs([
                format!("{}:/app", host_dir1.display()).parse()?,
                format!("{}:/data", host_dir2.display()).parse()?,
            ])
            .build();

        assert!(valid_config.validate().is_ok());

        // Test configuration with conflicting guest paths
        let invalid_config = MicroVmConfig::builder()
            .root_path(temp_dir.path())
            .ram_mib(1024)
            .mapped_dirs([
                format!("{}:/app/data", host_dir1.display()).parse()?,
                format!("{}:/app", host_dir2.display()).parse()?,
            ])
            .build();

        assert!(matches!(
            invalid_config.validate(),
            Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::ConflictingGuestPaths(_, _)
            ))
        ));

        Ok(())
    }
}

use std::{ffi::CString, net::Ipv4Addr, path::PathBuf, ptr};

use getset::Getters;
use ipnetwork::Ipv4Network;
use monoutils::SupportedPathType;
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{EnvPair, NetworkScope, PathPair, PortPair},
    utils, InvalidMicroVMConfigError, MonocoreError, MonocoreResult,
};

use super::{ffi, LinuxRlimit, MicroVmBuilder, MicroVmConfigBuilder};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The prefix used for virtio-fs tags when mounting shared directories
pub const VIRTIOFS_TAG_PREFIX: &str = "virtiofs";

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
/// ```no_run
/// use monocore::vm::{MicroVm, Rootfs};
/// use tempfile::TempDir;
///
/// # fn main() -> anyhow::Result<()> {
/// let temp_dir = TempDir::new()?;
/// let vm = MicroVm::builder()
///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
///     .ram_mib(1024)
///     .exec_path("/bin/echo")
///     .args(["Hello, World!"])
///     .build()?;
///
/// // Start the MicroVm
/// vm.start()?;  // This would actually run the VM
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Getters)]
pub struct MicroVm {
    /// The context ID for the MicroVm configuration.
    ctx_id: u32,

    /// The configuration for the MicroVm.
    #[get = "pub with_prefix"]
    config: MicroVmConfig,
}

/// The type of rootfs to use for the MicroVm.
///
/// This enum represents the different types of rootfss that can be used for the MicroVm.
///
/// ## Variants
///
/// * `Native(PathBuf)` - A native rootfs using a single path.
/// * `Overlayfs(Vec<PathBuf>)` - An overlayfs rootfs using a list of paths.
///
/// ## Examples
///
/// ```rust
/// use monocore::vm::Rootfs;
/// use std::path::PathBuf;
///
/// let native_root = Rootfs::Native(PathBuf::from("/path/to/root"));
/// let overlayfs_root = Rootfs::Overlayfs(vec![PathBuf::from("/path/to/root1"), PathBuf::from("/path/to/root2")]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rootfs {
    /// A rootfs using underlying native filesystem.
    Native(PathBuf),

    /// An overlayfs rootfs using a list of paths.
    Overlayfs(Vec<PathBuf>),
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
/// use monocore::vm::{MicroVm, MicroVmConfig, Rootfs};
/// use tempfile::TempDir;
///
/// # fn main() -> anyhow::Result<()> {
/// let temp_dir = TempDir::new()?;
/// let config = MicroVmConfig::builder()
///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
///     .ram_mib(1024)
///     .exec_path("/bin/echo")
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

    /// The rootfs for the MicroVm.
    pub rootfs: Rootfs,

    /// The number of vCPUs to use for the MicroVm.
    pub num_vcpus: u8,

    /// The amount of RAM in MiB to use for the MicroVm.
    pub ram_mib: u32,

    /// The directories to mount in the MicroVm using virtio-fs.
    /// Each PathPair represents a host:guest path mapping.
    pub mapped_dirs: Vec<PathPair>,

    /// The port map to use for the MicroVm.
    pub port_map: Vec<PortPair>,

    /// The network scope to use for the MicroVm.
    pub scope: NetworkScope,

    /// The IP address to use for the MicroVm.
    pub ip: Option<Ipv4Addr>,

    /// The subnet to use for the MicroVm.
    pub subnet: Option<Ipv4Network>,

    /// The resource limits to use for the MicroVm.
    pub rlimits: Vec<LinuxRlimit>,

    /// The working directory path to use for the MicroVm.
    pub workdir_path: Option<Utf8UnixPathBuf>,

    /// The executable path to use for the MicroVm.
    pub exec_path: Utf8UnixPathBuf,

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
    /// use monocore::vm::{MicroVm, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVm::builder()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)
    ///     .exec_path("/bin/echo")
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
    /// use monocore::vm::{MicroVm, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVm::builder()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
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

        // Set rootfs.
        match &config.rootfs {
            Rootfs::Native(path) => {
                let c_path = CString::new(path.to_str().unwrap().as_bytes()).unwrap();
                unsafe {
                    let status = ffi::krun_set_root(ctx_id, c_path.as_ptr());
                    assert!(status >= 0, "failed to set rootfs: {}", status);
                }
            }
            Rootfs::Overlayfs(paths) => {
                tracing::debug!("setting overlayfs rootfs: {:?}", paths);
                let c_paths: Vec<_> = paths
                    .iter()
                    .map(|p| CString::new(p.to_str().unwrap().as_bytes()).unwrap())
                    .collect();
                let c_paths_ptrs = utils::to_null_terminated_c_array(&c_paths);
                unsafe {
                    let status = ffi::krun_set_overlayfs_root(ctx_id, c_paths_ptrs.as_ptr());
                    assert!(status >= 0, "failed to set rootfs: {}", status);
                }
            }
        }

        // Add mapped directories using virtio-fs
        let mapped_dirs = &config.mapped_dirs;
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

        // Set network scope
        unsafe {
            let status = ffi::krun_set_tsi_scope(ctx_id, ptr::null(), ptr::null(), config.scope as u8);
            assert!(status >= 0, "failed to set network scope: {}", status);
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
        let c_exec_path = CString::new(config.exec_path.to_string().as_bytes()).unwrap();

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
    /// use monocore::vm::{MicroVmConfig, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let config = MicroVmConfig::builder()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)
    ///     .exec_path("/bin/echo")
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
    /// use monocore::vm::{MicroVmConfig, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let config = MicroVmConfig::builder()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)
    ///     .exec_path("/bin/echo")
    ///     .build();
    ///
    /// assert!(config.validate().is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> MonocoreResult<()> {
        // Check that paths specified in rootfs exist
        match &self.rootfs {
            Rootfs::Native(path) => {
                if !path.exists() {
                    return Err(MonocoreError::InvalidMicroVMConfig(
                        InvalidMicroVMConfigError::RootPathDoesNotExist(
                            path.to_str().unwrap().into(),
                        ),
                    ));
                }
            }
            Rootfs::Overlayfs(paths) => {
                for path in paths {
                    if !path.exists() {
                        return Err(MonocoreError::InvalidMicroVMConfig(
                            InvalidMicroVMConfigError::RootPathDoesNotExist(
                                path.to_str().unwrap().into(),
                            ),
                        ));
                    }
                }
            }
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

        Self::validate_command_line(self.exec_path.as_ref())?;

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
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_microvm_config_builder() {
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .rootfs(Rootfs::Native(PathBuf::from("/tmp")))
            .ram_mib(512)
            .exec_path("/bin/echo")
            .build();

        assert!(config.log_level == LogLevel::Info);
        assert_eq!(config.rootfs, Rootfs::Native(PathBuf::from("/tmp")));
        assert_eq!(config.ram_mib, 512);
        assert_eq!(config.num_vcpus, DEFAULT_NUM_VCPUS);
    }

    #[test]
    fn test_microvm_config_validation_success() {
        let temp_dir = TempDir::new().unwrap();
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
            .exec_path("/bin/echo")
            .build();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_microvm_config_validation_failure_root_path() {
        let config = MicroVmConfig::builder()
            .log_level(LogLevel::Info)
            .rootfs(Rootfs::Native(PathBuf::from("/non/existent/path")))
            .ram_mib(512)
            .exec_path("/bin/echo")
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
            .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
            .ram_mib(0)
            .exec_path("/bin/echo")
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
            .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
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
            .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
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
            .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
            .ram_mib(1024)
            .exec_path("/bin/echo")
            .mapped_dirs([
                format!("{}:/app", host_dir1.display()).parse()?,
                format!("{}:/data", host_dir2.display()).parse()?,
            ])
            .build();

        assert!(valid_config.validate().is_ok());

        // Test configuration with conflicting guest paths
        let invalid_config = MicroVmConfig::builder()
            .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
            .ram_mib(1024)
            .exec_path("/bin/echo")
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

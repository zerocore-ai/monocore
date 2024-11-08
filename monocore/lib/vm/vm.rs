use std::{ffi::CString, net::Ipv4Addr, path::PathBuf};

use getset::Getters;
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{EnvPair, PathPair, PortPair},
    utils, InvalidMicroVMConfigError, MonocoreError, MonocoreResult,
};

use super::{ffi, LinuxRlimit, MicroVmBuilder, MicroVmConfigBuilder};

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
/// use monocore::vm::MicroVmConfig;
/// use tempfile::TempDir;
///
/// # fn main() -> anyhow::Result<()> {
/// let temp_dir = TempDir::new()?;
/// let config = MicroVmConfig::builder()
///     .root_path(temp_dir.path())
///     .ram_mib(1024)
///     .build()?;
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

    /// The virtio-fs mounts to use for the MicroVm.
    pub virtiofs: Vec<PathPair>,

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

    /// The assigned IP address for the MicroVm.
    pub assigned_ip: Option<Ipv4Addr>,

    /// Whether the MicroVm is restricted to local connections only.
    pub local_only: bool,
}

/// The log level to use for the MicroVm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u32)]
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
    ///     .exec_path("/bin/true")
    ///     .build()?;
    ///
    /// // // Start the MicroVm
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
            tracing::error!("Failed to start micro VM: {}", status);
            return Err(MonocoreError::StartVmFailed(status));
        }
        tracing::info!("Micro VM exited with status: {}", status);
        Ok(status)
    }

    fn create_ctx() -> u32 {
        let ctx_id = unsafe { ffi::krun_create_ctx() };
        assert!(ctx_id >= 0, "Failed to create micro VM context: {}", ctx_id);
        ctx_id as u32
    }

    fn apply_config(ctx_id: u32, config: &MicroVmConfig) {
        // Set log level
        unsafe {
            let status = ffi::krun_set_log_level(config.log_level as u32);
            assert!(status >= 0, "Failed to set log level: {}", status);
        }

        // Set basic VM configuration
        unsafe {
            let status = ffi::krun_set_vm_config(ctx_id, config.num_vcpus, config.ram_mib);
            assert!(status >= 0, "Failed to set VM config: {}", status);
        }

        // Set root path
        let c_root_path = CString::new(config.root_path.to_str().unwrap().as_bytes()).unwrap();
        unsafe {
            let status = ffi::krun_set_root(ctx_id, c_root_path.as_ptr());
            assert!(status >= 0, "Failed to set root path: {}", status);
        }

        // Add virtio-fs mounts
        for mount in &config.virtiofs {
            let tag = CString::new(mount.get_guest().to_string().as_bytes()).unwrap();
            let path = CString::new(mount.get_host().to_string().as_bytes()).unwrap();
            unsafe {
                let status = ffi::krun_add_virtiofs(ctx_id, tag.as_ptr(), path.as_ptr());
                assert!(status >= 0, "Failed to add virtio-fs mount: {}", status);
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
            assert!(status >= 0, "Failed to set port map: {}", status);
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
                assert!(status >= 0, "Failed to set resource limits: {}", status);
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

        // Set assigned IP if configured
        if let Some(assigned_ip) = &config.assigned_ip {
            let ip_str = assigned_ip.to_string();
            let c_assigned_ip = CString::new(ip_str).unwrap();
            unsafe {
                let status = ffi::krun_set_tsi_rewrite_ip(ctx_id, c_assigned_ip.as_ptr());
                assert!(status >= 0, "Failed to set assigned IP: {}", status);
            }
        }

        // Set local_only mode
        unsafe {
            let status = ffi::krun_enable_tsi_local_only(ctx_id, config.local_only);
            assert!(status >= 0, "Failed to set local only mode: {}", status);
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
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> MicroVmConfigBuilder<(), ()> {
        MicroVmConfigBuilder::default()
    }

    /// Validates the MicroVm configuration.
    ///
    /// Performs a series of checks to ensure the configuration is valid:
    /// - Verifies the root path exists
    /// - Ensures number of vCPUs is non-zero
    /// - Ensures RAM allocation is non-zero
    /// - Validates executable path and arguments contain only printable ASCII characters
    ///
    /// ## Returns
    /// - `Ok(())` if the configuration is valid
    /// - `Err(MonocoreError::InvalidMicroVMConfig)` if any validation check fails
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
    ///     .build()?;
    ///
    /// assert!(config.validate().is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> MonocoreResult<()> {
        if !self.root_path.exists() {
            return Err(MonocoreError::InvalidMicroVMConfig(
                InvalidMicroVMConfigError::RootPathDoesNotExist(
                    self.root_path.to_str().unwrap().into(),
                ),
            ));
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

        Ok(())
    }

    /// Validates that a command line string contains only allowed characters.
    ///
    /// Command line strings (executable paths and arguments) must contain only printable ASCII
    /// characters in the range from space (0x20) to tilde (0x7E). This excludes control characters
    /// like newlines and tabs, as well as any non-ASCII Unicode characters.
    fn validate_command_line(s: &str) -> MonocoreResult<()> {
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
}

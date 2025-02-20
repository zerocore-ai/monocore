use std::path::PathBuf;

use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{EnvPair, PathPair, PortPair, DEFAULT_NUM_VCPUS},
    MonocoreResult,
};

use super::{LinuxRlimit, LogLevel, MicroVm, MicroVmConfig};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The builder for a MicroVm configuration.
#[derive(Debug)]
pub struct MicroVmConfigBuilder<RootPath, RamMib> {
    log_level: LogLevel,
    root_path: RootPath,
    num_vcpus: Option<u8>,
    ram_mib: RamMib,
    mapped_dirs: Vec<PathPair>,
    port_map: Vec<PortPair>,
    rlimits: Vec<LinuxRlimit>,
    workdir_path: Option<Utf8UnixPathBuf>,
    exec_path: Option<Utf8UnixPathBuf>,
    args: Vec<String>,
    env: Vec<EnvPair>,
    console_output: Option<Utf8UnixPathBuf>,
}

/// The builder for a MicroVm.
///
/// This struct provides a fluent interface for configuring and creating a `MicroVm` instance.
/// It allows you to set various parameters such as the log level, root path, number of vCPUs,
/// RAM size, virtio-fs mounts, port mappings, resource limits, working directory, executable path,
/// arguments, environment variables, and console output.
///
/// ## Examples
///
/// ```rust
/// use monocore::vm::{MicroVmBuilder, LogLevel};
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let vm = MicroVmBuilder::default()
///     .log_level(LogLevel::Debug)
///     .root_path("/tmp")
///     .num_vcpus(2)
///     .ram_mib(1024)
///     .mapped_dirs(["/home:/guest/mount".parse()?])
///     .port_map(["8080:80".parse()?])
///     .rlimits(["RLIMIT_NOFILE=1024:1024".parse()?])
///     .workdir_path("/workdir")
///     .exec_path("/bin/example")
///     .args(["arg1", "arg2"])
///     .env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?])
///     .console_output("/tmp/console.log")
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct MicroVmBuilder<RootPath, RamMib> {
    inner: MicroVmConfigBuilder<RootPath, RamMib>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<RootPath, RamMib> MicroVmConfigBuilder<RootPath, RamMib> {
    /// Sets the log level for the MicroVm.
    ///
    /// The log level controls the verbosity of the MicroVm's logging output.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmConfigBuilder, LogLevel};
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .log_level(LogLevel::Debug);  // Enable debug logging
    /// ```
    ///
    /// ## Log Levels
    /// - `Off` - No logging (default)
    /// - `Error` - Only error messages
    /// - `Warn` - Warnings and errors
    /// - `Info` - Informational messages, warnings, and errors
    /// - `Debug` - Debug information and all above
    /// - `Trace` - Detailed trace information and all above
    pub fn log_level(mut self, log_level: LogLevel) -> Self {
        self.log_level = log_level;
        self
    }

    /// Sets the root filesystem path for the MicroVm.
    ///
    /// This path serves as the root directory for the MicroVm's filesystem. It should contain
    /// all necessary files and directories for the guest system to operate.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .root_path("/path/to/alpine-rootfs");  // Use Alpine Linux root filesystem
    /// ```
    ///
    /// ## Notes
    /// - The path must exist and be accessible
    /// - The path should contain a valid root filesystem structure
    /// - Common choices include Alpine Linux or Ubuntu root filesystems
    pub fn root_path(self, root_path: impl Into<PathBuf>) -> MicroVmConfigBuilder<PathBuf, RamMib> {
        MicroVmConfigBuilder {
            log_level: self.log_level,
            root_path: root_path.into(),
            num_vcpus: self.num_vcpus,
            ram_mib: self.ram_mib,
            mapped_dirs: self.mapped_dirs,
            port_map: self.port_map,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            args: self.args,
            env: self.env,
            console_output: self.console_output,
        }
    }

    /// Sets the number of virtual CPUs (vCPUs) for the MicroVm.
    ///
    /// This determines how many CPU cores are available to the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .num_vcpus(2);  // Allocate 2 virtual CPU cores
    /// ```
    ///
    /// ## Notes
    /// - The default is 1 vCPU if not specified
    /// - The number of vCPUs should not exceed the host's physical CPU cores
    /// - More vCPUs aren't always better - consider the workload's needs
    pub fn num_vcpus(mut self, num_vcpus: u8) -> Self {
        self.num_vcpus = Some(num_vcpus);
        self
    }

    /// Sets the amount of RAM in MiB for the MicroVm.
    ///
    /// This determines how much memory is available to the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .ram_mib(1024);  // Allocate 1 GiB of RAM
    /// ```
    ///
    /// ## Notes
    /// - The value is in MiB (1 GiB = 1024 MiB)
    /// - Consider the host's available memory when setting this value
    /// - Common values: 512 MiB for minimal systems, 1024-2048 MiB for typical workloads
    pub fn ram_mib(self, ram_mib: u32) -> MicroVmConfigBuilder<RootPath, u32> {
        MicroVmConfigBuilder {
            log_level: self.log_level,
            root_path: self.root_path,
            num_vcpus: self.num_vcpus,
            ram_mib,
            mapped_dirs: self.mapped_dirs,
            port_map: self.port_map,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            args: self.args,
            env: self.env,
            console_output: self.console_output,
        }
    }

    /// Sets the directory mappings for the MicroVm using virtio-fs.
    ///
    /// Each mapping follows Docker's volume mapping convention using the format `host:guest`.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = MicroVmConfigBuilder::default()
    ///     .mapped_dirs([
    ///         // Share host's /data directory as /mnt/data in guest
    ///         "/data:/mnt/data".parse()?,
    ///         // Share current directory as /app in guest
    ///         "./:/app".parse()?,
    ///         // Use same path in both host and guest
    ///         "/shared".parse()?
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - Host paths must exist and be accessible
    /// - Guest paths will be created if they don't exist
    /// - Changes in shared directories are immediately visible to both systems
    /// - Useful for development, configuration files, and data sharing
    pub fn mapped_dirs(mut self, mapped_dirs: impl IntoIterator<Item = PathPair>) -> Self {
        self.mapped_dirs = mapped_dirs.into_iter().collect();
        self
    }

    /// Sets the port mappings between host and guest for the MicroVm.
    ///
    /// Port mappings follow Docker's convention using the format `host:guest`, where:
    /// - `host` is the port number on the host machine
    /// - `guest` is the port number inside the MicroVm
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    /// use monocore::config::PortPair;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = MicroVmConfigBuilder::default()
    ///     .port_map([
    ///         // Map host port 8080 to guest port 80
    ///         "8080:80".parse()?,
    ///         // Map host port 2222 to guest port 22
    ///         "2222:22".parse()?,
    ///         // Use same port (3000) on both host and guest
    ///         "3000".parse()?
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    ///
    /// - If you don't call this method, no ports will be mapped between host and guest
    /// - The guest application will need to use the guest port number to listen for connections
    /// - External connections should use the host port number to connect to the service
    pub fn port_map(mut self, port_map: impl IntoIterator<Item = PortPair>) -> Self {
        self.port_map = port_map.into_iter().collect();
        self
    }

    /// Sets the resource limits for processes in the MicroVm.
    ///
    /// Resource limits control various system resources available to processes running
    /// in the guest system, following Linux's rlimit convention.
    ///
    /// ## Format
    /// Resource limits use the format `RESOURCE=SOFT:HARD` or `NUMBER=SOFT:HARD`, where:
    /// - `RESOURCE` is the resource name (e.g., RLIMIT_NOFILE)
    /// - `NUMBER` is the resource number (e.g., 7 for RLIMIT_NOFILE)
    /// - `SOFT` is the soft limit (enforced limit)
    /// - `HARD` is the hard limit (ceiling for soft limit)
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = MicroVmConfigBuilder::default()
    ///     .rlimits([
    ///         // Limit number of open files
    ///         "RLIMIT_NOFILE=1024:2048".parse()?,
    ///         // Limit process memory
    ///         "RLIMIT_AS=1073741824:2147483648".parse()?,  // 1GB:2GB
    ///         // Can also use resource numbers
    ///         "7=1024:2048".parse()?  // Same as RLIMIT_NOFILE
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Common Resource Limits
    /// - `RLIMIT_NOFILE` (7) - Maximum number of open files
    /// - `RLIMIT_AS` (9) - Maximum size of process's virtual memory
    /// - `RLIMIT_NPROC` (6) - Maximum number of processes
    /// - `RLIMIT_CPU` (0) - CPU time limit in seconds
    /// - `RLIMIT_FSIZE` (1) - Maximum file size
    pub fn rlimits(mut self, rlimits: impl IntoIterator<Item = LinuxRlimit>) -> Self {
        self.rlimits = rlimits.into_iter().collect();
        self
    }

    /// Sets the working directory for processes in the MicroVm.
    ///
    /// This directory will be the current working directory (cwd) for any processes
    /// started in the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .workdir_path("/app")  // Set working directory to /app
    ///     .exec_path("/app/myapp")  // Run executable from /app
    ///     .args(["--config", "config.json"]);  // Config file will be looked up in /app
    /// ```
    ///
    /// ## Notes
    /// - The path must be absolute
    /// - The directory must exist in the guest filesystem
    /// - Useful for applications that need to access files relative to their location
    pub fn workdir_path(mut self, workdir_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.workdir_path = Some(workdir_path.into());
        self
    }

    /// Sets the path to the executable to run in the MicroVm.
    ///
    /// This specifies the program that will be executed when the MicroVm starts.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .exec_path("/usr/local/bin/nginx")  // Run nginx web server
    ///     .args(["-c", "/etc/nginx/nginx.conf"]);  // With specific config
    /// ```
    ///
    /// ## Notes
    /// - The path must be absolute
    /// - The executable must exist and be executable in the guest filesystem
    /// - The path is relative to the guest's root filesystem
    pub fn exec_path(mut self, exec_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.exec_path = Some(exec_path.into());
        self
    }

    /// Sets the command-line arguments for the executable.
    ///
    /// These arguments will be passed to the program specified by `exec_path`.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .exec_path("/usr/bin/python3")
    ///     .args([
    ///         "-m", "http.server",  // Run Python's HTTP server module
    ///         "8080",               // Listen on port 8080
    ///         "--directory", "/data" // Serve files from /data
    ///     ]);
    /// ```
    ///
    /// ## Notes
    /// - Arguments are passed in the order they appear in the iterator
    /// - The program name (argv[0]) is automatically set from exec_path
    /// - Each argument should be a separate string
    pub fn args<'a>(mut self, args: impl IntoIterator<Item = &'a str>) -> Self {
        self.args = args.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// Sets environment variables for processes in the MicroVm.
    ///
    /// Environment variables follow the standard format `KEY=VALUE` and are available
    /// to all processes in the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = MicroVmConfigBuilder::default()
    ///     .env([
    ///         // Set application environment
    ///         "APP_ENV=production".parse()?,
    ///         // Configure logging
    ///         "LOG_LEVEL=info".parse()?,
    ///         // Set timezone
    ///         "TZ=UTC".parse()?,
    ///         // Multiple values are OK
    ///         "ALLOWED_HOSTS=localhost,127.0.0.1".parse()?
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - Variables are available to all processes in the guest
    /// - Values should be properly escaped if they contain special characters
    /// - Common uses include configuration and runtime settings
    /// - Some programs expect specific environment variables to function
    pub fn env(mut self, env: impl IntoIterator<Item = EnvPair>) -> Self {
        self.env = env.into_iter().collect();
        self
    }

    /// Sets the path for capturing console output from the MicroVm.
    ///
    /// This allows redirecting and saving all console output (stdout/stderr) from
    /// the guest system to a file on the host.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .console_output("/var/log/microvm.log")  // Save output to log file
    ///     .exec_path("/usr/local/bin/myapp");      // Run application
    /// ```
    ///
    /// ## Notes
    /// - The path must be writable on the host system
    /// - The file will be created if it doesn't exist
    /// - Useful for debugging and logging
    /// - Captures both stdout and stderr
    pub fn console_output(mut self, console_output: impl Into<Utf8UnixPathBuf>) -> Self {
        self.console_output = Some(console_output.into());
        self
    }
}

impl<RootPath, RamMib> MicroVmBuilder<RootPath, RamMib> {
    /// Sets the log level for the MicroVm.
    ///
    /// The log level controls the verbosity of the MicroVm's logging output.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{LogLevel, MicroVmBuilder};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .log_level(LogLevel::Debug)  // Enable debug logging
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Log Levels
    /// - `Off` - No logging (default)
    /// - `Error` - Only error messages
    /// - `Warn` - Warnings and errors
    /// - `Info` - Informational messages, warnings, and errors
    /// - `Debug` - Debug information and all above
    /// - `Trace` - Detailed trace information and all above
    pub fn log_level(mut self, log_level: LogLevel) -> Self {
        self.inner = self.inner.log_level(log_level);
        self
    }

    /// Sets the root filesystem path for the MicroVm.
    ///
    /// This path serves as the root directory for the MicroVm's filesystem. It should contain
    /// all necessary files and directories for the guest system to operate.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .root_path(temp_dir.path())  // Use temporary root filesystem
    ///     .ram_mib(1024)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The path must exist and be accessible
    /// - The path should contain a valid root filesystem structure
    /// - Common choices include Alpine Linux or Ubuntu root filesystems
    /// - This is a required field - the build will fail if not set
    pub fn root_path(self, root_path: impl Into<PathBuf>) -> MicroVmBuilder<PathBuf, RamMib> {
        MicroVmBuilder {
            inner: self.inner.root_path(root_path),
        }
    }

    /// Sets the number of virtual CPUs (vCPUs) for the MicroVm.
    ///
    /// This determines how many CPU cores are available to the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .num_vcpus(2)  // Allocate 2 virtual CPU cores
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The default is 1 vCPU if not specified
    /// - The number of vCPUs should not exceed the host's physical CPU cores
    /// - More vCPUs aren't always better - consider the workload's needs
    pub fn num_vcpus(mut self, num_vcpus: u8) -> Self {
        self.inner = self.inner.num_vcpus(num_vcpus);
        self
    }

    /// Sets the amount of RAM in MiB for the MicroVm.
    ///
    /// This determines how much memory is available to the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)  // Allocate 1 GiB of RAM
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The value is in MiB (1 GiB = 1024 MiB)
    /// - Consider the host's available memory when setting this value
    /// - Common values: 512 MiB for minimal systems, 1024-2048 MiB for typical workloads
    /// - This is a required field - the build will fail if not set
    pub fn ram_mib(self, ram_mib: u32) -> MicroVmBuilder<RootPath, u32> {
        MicroVmBuilder {
            inner: self.inner.ram_mib(ram_mib),
        }
    }

    /// Sets the directory mappings for the MicroVm using virtio-fs.
    ///
    /// Each mapping follows Docker's volume mapping convention using the format `host:guest`.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = MicroVmConfigBuilder::default()
    ///     .mapped_dirs([
    ///         // Share host's /data directory as /mnt/data in guest
    ///         "/data:/mnt/data".parse()?,
    ///         // Share current directory as /app in guest
    ///         "./:/app".parse()?,
    ///         // Use same path in both host and guest
    ///         "/shared".parse()?
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn mapped_dirs(mut self, mapped_dirs: impl IntoIterator<Item = PathPair>) -> Self {
        self.inner = self.inner.mapped_dirs(mapped_dirs);
        self
    }

    /// Sets the port mappings between host and guest for the MicroVm.
    ///
    /// Port mappings follow Docker's convention using the format `host:guest`, where:
    /// - `host` is the port number on the host machine
    /// - `guest` is the port number inside the MicroVm
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    /// use monocore::config::PortPair;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .port_map([
    ///         // Map host port 8080 to guest port 80 (for web server)
    ///         "8080:80".parse()?,
    ///         // Map host port 2222 to guest port 22 (for SSH)
    ///         "2222:22".parse()?,
    ///         // Use same port (3000) on both host and guest
    ///         "3000".parse()?
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    ///
    /// - If you don't call this method, no ports will be mapped between host and guest
    /// - The guest application will need to use the guest port number to listen for connections
    /// - External connections should use the host port number to connect to the service
    /// - Port mapping is not supported when using passt networking mode
    pub fn port_map(mut self, port_map: impl IntoIterator<Item = PortPair>) -> Self {
        self.inner = self.inner.port_map(port_map);
        self
    }

    /// Sets the resource limits for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmBuilder::default().rlimits(["RLIMIT_NOFILE=1024:1024".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn rlimits(mut self, rlimits: impl IntoIterator<Item = LinuxRlimit>) -> Self {
        self.inner = self.inner.rlimits(rlimits);
        self
    }

    /// Sets the working directory path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().workdir_path("/path/to/workdir");
    /// ```
    pub fn workdir_path(mut self, workdir_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.inner = self.inner.workdir_path(workdir_path);
        self
    }

    /// Sets the executable path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().exec_path("/path/to/exec");
    /// ```
    pub fn exec_path(mut self, exec_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.inner = self.inner.exec_path(exec_path);
        self
    }

    /// Sets the command-line arguments for the executable.
    ///
    /// These arguments will be passed to the program specified by `exec_path`.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .args([
    ///         "-m", "http.server",  // Run Python's HTTP server module
    ///         "8080",               // Listen on port 8080
    ///         "--directory", "/data" // Serve files from /data
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - Arguments are passed in the order they appear in the iterator
    /// - The program name (argv[0]) is automatically set from exec_path
    /// - Each argument should be a separate string
    pub fn args<'a>(mut self, args: impl IntoIterator<Item = &'a str>) -> Self {
        self.inner = self.inner.args(args);
        self
    }

    /// Sets environment variables for processes in the MicroVm.
    ///
    /// Environment variables follow the standard format `KEY=VALUE` and are available
    /// to all processes in the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .env([
    ///         // Set application environment
    ///         "APP_ENV=production".parse()?,
    ///         // Configure logging
    ///         "LOG_LEVEL=info".parse()?,
    ///         // Set timezone
    ///         "TZ=UTC".parse()?,
    ///         // Multiple values are OK
    ///         "ALLOWED_HOSTS=localhost,127.0.0.1".parse()?
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - Variables are available to all processes in the guest
    /// - Values should be properly escaped if they contain special characters
    /// - Common uses include configuration and runtime settings
    /// - Some programs expect specific environment variables to function
    pub fn env(mut self, env: impl IntoIterator<Item = EnvPair>) -> Self {
        self.inner = self.inner.env(env);
        self
    }

    /// Sets the path for capturing console output from the MicroVm.
    ///
    /// This allows redirecting and saving all console output (stdout/stderr) from
    /// the guest system to a file on the host.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .console_output("/var/log/microvm.log")  // Save output to log file
    ///     .exec_path("/usr/local/bin/myapp");      // Run application
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The path must be writable on the host system
    /// - The file will be created if it doesn't exist
    /// - Useful for debugging and logging
    /// - Captures both stdout and stderr
    pub fn console_output(mut self, console_output: impl Into<Utf8UnixPathBuf>) -> Self {
        self.inner = self.inner.console_output(console_output);
        self
    }
}

impl MicroVmConfigBuilder<PathBuf, u32> {
    /// Builds the MicroVm configuration.
    pub fn build(self) -> MicroVmConfig {
        MicroVmConfig {
            log_level: self.log_level,
            root_path: self.root_path,
            num_vcpus: self.num_vcpus.unwrap_or(DEFAULT_NUM_VCPUS),
            ram_mib: self.ram_mib,
            mapped_dirs: self.mapped_dirs,
            port_map: self.port_map,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            args: self.args,
            env: self.env,
            console_output: self.console_output,
        }
    }
}

impl MicroVmBuilder<PathBuf, u32> {
    /// Builds the MicroVm.
    ///
    /// This method creates a `MicroVm` instance based on the configuration set in the builder.
    /// The MicroVm will be ready to start but won't be running until you call `start()`.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmBuilder;
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .root_path(temp_dir.path())
    ///     .ram_mib(1024)
    ///     .exec_path("/usr/bin/python3")
    ///     .args(["-c", "print('Hello from MicroVm!')"])
    ///     .build()?;
    ///
    /// // // Start the MicroVm
    /// // vm.start()?;  // This would actually run the VM
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Required Configuration
    /// - `root_path` - Path to the root filesystem
    /// - `ram_mib` - Amount of RAM to allocate
    ///
    /// ## Notes
    /// - The build will fail if required configuration is missing
    /// - The build will fail if the root path doesn't exist
    /// - The build will fail if RAM or vCPU values are invalid
    /// - After building, use `start()` to run the MicroVm
    pub fn build(self) -> MonocoreResult<MicroVm> {
        MicroVm::from_config(MicroVmConfig {
            log_level: self.inner.log_level,
            root_path: self.inner.root_path,
            num_vcpus: self.inner.num_vcpus.unwrap_or(DEFAULT_NUM_VCPUS),
            ram_mib: self.inner.ram_mib,
            mapped_dirs: self.inner.mapped_dirs,
            port_map: self.inner.port_map,
            rlimits: self.inner.rlimits,
            workdir_path: self.inner.workdir_path,
            exec_path: self.inner.exec_path,
            args: self.inner.args,
            env: self.inner.env,
            console_output: self.inner.console_output,
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for MicroVmConfigBuilder<(), ()> {
    fn default() -> Self {
        Self {
            log_level: LogLevel::default(),
            root_path: (),
            num_vcpus: Some(DEFAULT_NUM_VCPUS),
            ram_mib: (),
            mapped_dirs: vec![],
            port_map: vec![],
            rlimits: vec![],
            workdir_path: None,
            exec_path: None,
            args: vec![],
            env: vec![],
            console_output: None,
        }
    }
}

impl Default for MicroVmBuilder<(), ()> {
    fn default() -> Self {
        Self {
            inner: MicroVmConfigBuilder::default(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_microvm_builder() -> anyhow::Result<()> {
        let root_path = "/tmp";
        let workdir_path = "/workdir";
        let exec_path = "/bin/example";

        let builder = MicroVmBuilder::default()
            .log_level(LogLevel::Debug)
            .root_path(root_path)
            .num_vcpus(2)
            .ram_mib(1024)
            .mapped_dirs(["/guest/mount:/host/mount".parse()?])
            .port_map(["8080:80".parse()?])
            .rlimits(["RLIMIT_NOFILE=1024:1024".parse()?])
            .workdir_path(workdir_path)
            .exec_path(exec_path)
            .args(["arg1", "arg2"])
            .env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?])
            .console_output("/tmp/console.log");

        assert_eq!(builder.inner.log_level, LogLevel::Debug);
        assert_eq!(builder.inner.root_path, PathBuf::from(root_path));
        assert_eq!(builder.inner.num_vcpus, Some(2));
        assert_eq!(builder.inner.ram_mib, 1024);
        assert_eq!(
            builder.inner.mapped_dirs,
            ["/guest/mount:/host/mount".parse()?]
        );
        assert_eq!(builder.inner.port_map, ["8080:80".parse()?]);
        assert_eq!(builder.inner.rlimits, ["RLIMIT_NOFILE=1024:1024".parse()?]);
        assert_eq!(
            builder.inner.workdir_path,
            Some(Utf8UnixPathBuf::from(workdir_path))
        );
        assert_eq!(
            builder.inner.exec_path,
            Some(Utf8UnixPathBuf::from(exec_path))
        );
        assert_eq!(builder.inner.args, ["arg1", "arg2"]);
        assert_eq!(
            builder.inner.env,
            ["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?]
        );
        assert_eq!(
            builder.inner.console_output,
            Some(Utf8UnixPathBuf::from("/tmp/console.log"))
        );
        Ok(())
    }

    #[test]
    fn test_microvm_builder_minimal() -> anyhow::Result<()> {
        let root_path = "/tmp";
        let ram_mib = 512;

        let builder = MicroVmBuilder::default()
            .root_path(root_path)
            .ram_mib(ram_mib);

        assert_eq!(builder.inner.root_path, PathBuf::from(root_path));
        assert_eq!(builder.inner.ram_mib, ram_mib);

        // Check that other fields have default values
        assert_eq!(builder.inner.log_level, LogLevel::default());
        assert_eq!(builder.inner.num_vcpus, Some(DEFAULT_NUM_VCPUS));
        assert!(builder.inner.mapped_dirs.is_empty());
        assert!(builder.inner.port_map.is_empty());
        assert!(builder.inner.rlimits.is_empty());
        assert_eq!(builder.inner.workdir_path, None);
        assert_eq!(builder.inner.exec_path, None);
        assert!(builder.inner.args.is_empty());
        assert!(builder.inner.env.is_empty());
        assert_eq!(builder.inner.console_output, None);
        Ok(())
    }
}

use std::net::Ipv4Addr;

use ipnetwork::Ipv4Network;
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{EnvPair, NetworkScope, PathPair, PortPair, DEFAULT_NUM_VCPUS, DEFAULT_RAM_MIB},
    MonocoreResult,
};

use super::{LinuxRlimit, LogLevel, MicroVm, MicroVmConfig, Rootfs};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The builder for a MicroVm configuration.
///
/// ## Required Fields
/// - `rootfs`: The root filesystem to use for the MicroVm.
/// - `exec_path`: The path to the executable to run in the MicroVm.
///
/// ## Optional Fields
/// - `num_vcpus`: The number of virtual CPUs to use for the MicroVm.
/// - `ram_mib`: The amount of RAM in MiB to use for the MicroVm.
/// - `mapped_dirs`: The directories to mount in the MicroVm.
/// - `port_map`: The ports to map in the MicroVm.
/// - `rlimits`: The resource limits to use for the MicroVm.
/// - `workdir_path`: The working directory to use for the MicroVm.
/// - `args`: The arguments to pass to the executable.
/// - `env`: The environment variables to use for the MicroVm.
/// - `console_output`: The path to the file to write the console output to.
#[derive(Debug)]
pub struct MicroVmConfigBuilder<R, E> {
    log_level: LogLevel,
    rootfs: R,
    num_vcpus: u8,
    ram_mib: u32,
    mapped_dirs: Vec<PathPair>,
    port_map: Vec<PortPair>,
    scope: NetworkScope,
    ip: Option<Ipv4Addr>,
    subnet: Option<Ipv4Network>,
    rlimits: Vec<LinuxRlimit>,
    workdir_path: Option<Utf8UnixPathBuf>,
    exec_path: E,
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
/// ## Required Fields
/// - `rootfs`: The root filesystem to use for the MicroVm.
/// - `exec_path`: The path to the executable to run in the MicroVm.
///
/// ## Optional Fields
/// - `num_vcpus`: The number of virtual CPUs to use for the MicroVm.
/// - `ram_mib`: The amount of RAM in MiB to use for the MicroVm.
/// - `mapped_dirs`: The directories to mount in the MicroVm.
/// - `port_map`: The ports to map in the MicroVm.
/// - `scope`: The network scope to use for the MicroVm.
/// - `ip`: The IP address to use for the MicroVm.
/// - `subnet`: The subnet to use for the MicroVm.
/// - `rlimits`: The resource limits to use for the MicroVm.
/// - `workdir_path`: The working directory to use for the MicroVm.
/// - `args`: The arguments to pass to the executable.
/// - `env`: The environment variables to use for the MicroVm.
/// - `console_output`: The path to the file to write the console output to.
///
/// ## Examples
///
/// ```rust
/// use monocore::vm::{MicroVmBuilder, LogLevel, Rootfs};
/// use monocore::config::NetworkScope;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let vm = MicroVmBuilder::default()
///     .log_level(LogLevel::Debug)
///     .rootfs(Rootfs::Native(PathBuf::from("/tmp")))
///     .num_vcpus(2)
///     .ram_mib(1024)
///     .mapped_dirs(["/home:/guest/mount".parse()?])
///     .port_map(["8080:80".parse()?])
///     .scope(NetworkScope::Public)
///     .ip("192.168.1.100".parse()?)
///     .subnet("192.168.1.0/24".parse()?)
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
pub struct MicroVmBuilder<R, E> {
    inner: MicroVmConfigBuilder<R, E>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<R, M> MicroVmConfigBuilder<R, M> {
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

    /// Sets the root filesystem sharing mode for the MicroVm.
    ///
    /// This determines how the root filesystem is shared with the guest system, with two options:
    /// - `Rootfs::Native`: Direct passthrough of a directory as the root filesystem
    /// - `Rootfs::Overlayfs`: Use overlayfs with multiple layers as the root filesystem
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmConfigBuilder, Rootfs};
    /// use std::path::PathBuf;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     // Option 1: Direct passthrough of a directory
    ///     .rootfs(Rootfs::Native(PathBuf::from("/path/to/rootfs")));
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     // Option 2: Overlayfs with multiple layers
    ///     .rootfs(Rootfs::Overlayfs(vec![
    ///         PathBuf::from("/path/to/layer1"),
    ///         PathBuf::from("/path/to/layer2")
    ///     ]));
    /// ```
    ///
    /// ## Notes
    /// - For Passthrough: The directory must exist and contain a valid root filesystem structure
    /// - For Overlayfs: The layers are stacked in order, with later layers taking precedence
    /// - Common choices include Alpine Linux or Ubuntu root filesystems
    pub fn rootfs(self, rootfs: Rootfs) -> MicroVmConfigBuilder<Rootfs, M> {
        MicroVmConfigBuilder {
            log_level: self.log_level,
            rootfs,
            num_vcpus: self.num_vcpus,
            ram_mib: self.ram_mib,
            mapped_dirs: self.mapped_dirs,
            port_map: self.port_map,
            scope: self.scope,
            ip: self.ip,
            subnet: self.subnet,
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
        self.num_vcpus = num_vcpus;
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
    pub fn ram_mib(mut self, ram_mib: u32) -> Self {
        self.ram_mib = ram_mib;
        self
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

    /// Sets the network scope for the MicroVm.
    ///
    /// The network scope controls the MicroVm's level of network isolation and connectivity.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    /// use monocore::config::NetworkScope;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .scope(NetworkScope::Public);  // Allow access to public networks
    /// ```
    ///
    /// ## Network Scope Options
    /// - `None` - Sandboxes cannot communicate with any other sandboxes
    /// - `Group` - Sandboxes can only communicate within their subnet (default)
    /// - `Public` - Sandboxes can communicate with any other non-private address
    /// - `Any` - Sandboxes can communicate with any address
    ///
    /// ## Notes
    /// - Choose the appropriate scope based on your security requirements
    /// - More restrictive scopes provide better isolation
    /// - The default scope is `Group` if not specified
    pub fn scope(mut self, scope: NetworkScope) -> Self {
        self.scope = scope;
        self
    }

    /// Sets the IP address for the MicroVm.
    ///
    /// This sets a specific IPv4 address for the guest system's network interface.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    /// use std::net::Ipv4Addr;
    ///
    /// let config = MicroVmConfigBuilder::default()
    ///     .ip(Ipv4Addr::new(192, 168, 1, 100));  // Assign IP 192.168.1.100 to the MicroVm
    /// ```
    ///
    /// ## Notes
    /// - The IP address should be within the subnet assigned to the MicroVm
    /// - If not specified, an IP address may be assigned automatically
    /// - Useful for predictable addressing when running multiple MicroVms
    /// - Consider using with the `subnet` method to define the network
    pub fn ip(mut self, ip: Ipv4Addr) -> Self {
        self.ip = Some(ip);
        self
    }

    /// Sets the subnet for the MicroVm.
    ///
    /// This defines the IPv4 network and mask for the guest system's network interface.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::MicroVmConfigBuilder;
    /// use ipnetwork::Ipv4Network;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = MicroVmConfigBuilder::default()
    ///     .subnet("192.168.1.0/24".parse()?);  // Set subnet to 192.168.1.0/24
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The subnet defines the range of IP addresses available to the MicroVm
    /// - Common subnet masks: /24 (256 addresses), /16 (65536 addresses)
    /// - IP addresses assigned to the MicroVm should be within this subnet
    /// - Important for networking between multiple MicroVms in the same group
    pub fn subnet(mut self, subnet: Ipv4Network) -> Self {
        self.subnet = Some(subnet);
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
    pub fn exec_path(
        self,
        exec_path: impl Into<Utf8UnixPathBuf>,
    ) -> MicroVmConfigBuilder<R, Utf8UnixPathBuf> {
        MicroVmConfigBuilder {
            log_level: self.log_level,
            rootfs: self.rootfs,
            num_vcpus: self.num_vcpus,
            ram_mib: self.ram_mib,
            mapped_dirs: self.mapped_dirs,
            port_map: self.port_map,
            scope: self.scope,
            ip: self.ip,
            subnet: self.subnet,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: exec_path.into(),
            args: self.args,
            env: self.env,
            console_output: self.console_output,
        }
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

impl<R, M> MicroVmBuilder<R, M> {
    /// Sets the log level for the MicroVm.
    ///
    /// The log level controls the verbosity of the MicroVm's logging output.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{LogLevel, MicroVmBuilder, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .log_level(LogLevel::Debug)  // Enable debug logging
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)
    ///     .exec_path("/bin/echo")
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

    /// Sets the root filesystem sharing mode for the MicroVm.
    ///
    /// This determines how the root filesystem is shared with the guest system, with two options:
    /// - `Rootfs::Native`: Direct passthrough of a directory as the root filesystem
    /// - `Rootfs::Overlayfs`: Use overlayfs with multiple layers as the root filesystem
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use std::path::PathBuf;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// // Option 1: Direct passthrough
    /// let vm = MicroVmBuilder::default()
    ///     .rootfs(Rootfs::Native(PathBuf::from("/path/to/rootfs")));
    ///
    /// // Option 2: Overlayfs with layers
    /// let vm = MicroVmBuilder::default()
    ///     .rootfs(Rootfs::Overlayfs(vec![
    ///         PathBuf::from("/path/to/layer1"),
    ///         PathBuf::from("/path/to/layer2")
    ///     ]));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - For Passthrough: The directory must exist and contain a valid root filesystem structure
    /// - For Overlayfs: The layers are stacked in order, with later layers taking precedence
    /// - Common choices include Alpine Linux or Ubuntu root filesystems
    /// - This is a required field - the build will fail if not set
    pub fn rootfs(self, rootfs: Rootfs) -> MicroVmBuilder<Rootfs, M> {
        MicroVmBuilder {
            inner: self.inner.rootfs(rootfs),
        }
    }

    /// Sets the number of virtual CPUs (vCPUs) for the MicroVm.
    ///
    /// This determines how many CPU cores are available to the guest system.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)
    ///     .num_vcpus(2)  // Allocate 2 virtual CPU cores
    ///     .exec_path("/bin/echo")
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The default is 1 vCPU if not specified
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
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)  // Allocate 1 GiB of RAM
    ///     .exec_path("/bin/echo")
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The value is in MiB (1 GiB = 1024 MiB)
    /// - Consider the host's available memory when setting this value
    /// - This is a required field - the build will fail if not set
    pub fn ram_mib(mut self, ram_mib: u32) -> Self {
        self.inner = self.inner.ram_mib(ram_mib);
        self
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

    /// Sets the network scope for the MicroVm.
    ///
    /// The network scope controls the MicroVm's level of network isolation and connectivity.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use monocore::config::NetworkScope;
    /// use std::path::PathBuf;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .scope(NetworkScope::Public)  // Allow access to public networks
    ///     .rootfs(Rootfs::Native(PathBuf::from("/path/to/rootfs")))
    ///     .exec_path("/bin/echo");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Network Scope Options
    /// - `None` - Sandboxes cannot communicate with any other sandboxes
    /// - `Group` - Sandboxes can only communicate within their subnet (default)
    /// - `Public` - Sandboxes can communicate with any other non-private address
    /// - `Any` - Sandboxes can communicate with any address
    pub fn scope(mut self, scope: NetworkScope) -> Self {
        self.inner = self.inner.scope(scope);
        self
    }

    /// Sets the IP address for the MicroVm.
    ///
    /// This sets a specific IPv4 address for the guest system's network interface.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use std::path::PathBuf;
    /// use std::net::Ipv4Addr;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .ip(Ipv4Addr::new(192, 168, 1, 100))  // Assign IP 192.168.1.100 to the MicroVm
    ///     .rootfs(Rootfs::Native(PathBuf::from("/path/to/rootfs")))
    ///     .exec_path("/bin/echo");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The IP address should be within the subnet assigned to the MicroVm
    /// - If not specified, an IP address may be assigned automatically
    pub fn ip(mut self, ip: Ipv4Addr) -> Self {
        self.inner = self.inner.ip(ip);
        self
    }

    /// Sets the subnet for the MicroVm.
    ///
    /// This defines the IPv4 network and mask for the guest system's network interface.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use std::path::PathBuf;
    /// use ipnetwork::Ipv4Network;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmBuilder::default()
    ///     .subnet("192.168.1.0/24".parse()?)  // Set subnet to 192.168.1.0/24
    ///     .rootfs(Rootfs::Native(PathBuf::from("/path/to/rootfs")))
    ///     .exec_path("/bin/echo");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The subnet defines the range of IP addresses available to the MicroVm
    /// - Used for networking between multiple MicroVms in the same group
    pub fn subnet(mut self, subnet: Ipv4Network) -> Self {
        self.inner = self.inner.subnet(subnet);
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
    pub fn exec_path(
        self,
        exec_path: impl Into<Utf8UnixPathBuf>,
    ) -> MicroVmBuilder<R, Utf8UnixPathBuf> {
        MicroVmBuilder {
            inner: self.inner.exec_path(exec_path),
        }
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

impl MicroVmConfigBuilder<Rootfs, Utf8UnixPathBuf> {
    /// Builds the MicroVm configuration.
    pub fn build(self) -> MicroVmConfig {
        MicroVmConfig {
            log_level: self.log_level,
            rootfs: self.rootfs,
            num_vcpus: self.num_vcpus,
            ram_mib: self.ram_mib,
            mapped_dirs: self.mapped_dirs,
            port_map: self.port_map,
            scope: self.scope,
            ip: self.ip,
            subnet: self.subnet,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            args: self.args,
            env: self.env,
            console_output: self.console_output,
        }
    }
}

impl MicroVmBuilder<Rootfs, Utf8UnixPathBuf> {
    /// Builds the MicroVm.
    ///
    /// This method creates a `MicroVm` instance based on the configuration set in the builder.
    /// The MicroVm will be ready to start but won't be running until you call `start()`.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// use monocore::vm::{MicroVmBuilder, Rootfs};
    /// use tempfile::TempDir;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let temp_dir = TempDir::new()?;
    /// let vm = MicroVmBuilder::default()
    ///     .rootfs(Rootfs::Native(temp_dir.path().to_path_buf()))
    ///     .ram_mib(1024)
    ///     .exec_path("/usr/bin/python3")
    ///     .args(["-c", "print('Hello from MicroVm!')"])
    ///     .build()?;
    ///
    /// // Start the MicroVm
    /// vm.start()?;  // This would actually run the VM
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Notes
    /// - The build will fail if required configuration is missing
    /// - The build will fail if the root path doesn't exist
    /// - The build will fail if RAM or vCPU values are invalid
    /// - After building, use `start()` to run the MicroVm
    pub fn build(self) -> MonocoreResult<MicroVm> {
        MicroVm::from_config(MicroVmConfig {
            log_level: self.inner.log_level,
            rootfs: self.inner.rootfs,
            num_vcpus: self.inner.num_vcpus,
            ram_mib: self.inner.ram_mib,
            mapped_dirs: self.inner.mapped_dirs,
            port_map: self.inner.port_map,
            scope: self.inner.scope,
            ip: self.inner.ip,
            subnet: self.inner.subnet,
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
            rootfs: (),
            num_vcpus: DEFAULT_NUM_VCPUS,
            ram_mib: DEFAULT_RAM_MIB,
            mapped_dirs: vec![],
            port_map: vec![],
            scope: NetworkScope::Group,
            ip: None,
            subnet: None,
            rlimits: vec![],
            workdir_path: None,
            exec_path: (),
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
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_microvm_builder() -> anyhow::Result<()> {
        let rootfs = Rootfs::Overlayfs(vec![PathBuf::from("/tmp")]);
        let workdir_path = "/workdir";
        let exec_path = "/bin/example";

        let builder = MicroVmBuilder::default()
            .log_level(LogLevel::Debug)
            .rootfs(rootfs.clone())
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
        assert_eq!(builder.inner.rootfs, rootfs);
        assert_eq!(builder.inner.num_vcpus, 2);
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
        assert_eq!(builder.inner.exec_path, Utf8UnixPathBuf::from(exec_path));
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
        let rootfs = Rootfs::Native(PathBuf::from("/tmp"));
        let ram_mib = 1024;

        let builder = MicroVmBuilder::default()
            .rootfs(rootfs.clone())
            .exec_path("/bin/echo");

        assert_eq!(builder.inner.rootfs, rootfs);
        assert_eq!(builder.inner.ram_mib, ram_mib);

        // Check that other fields have default values
        assert_eq!(builder.inner.log_level, LogLevel::default());
        assert_eq!(builder.inner.num_vcpus, DEFAULT_NUM_VCPUS);
        assert_eq!(builder.inner.ram_mib, DEFAULT_RAM_MIB);
        assert!(builder.inner.mapped_dirs.is_empty());
        assert!(builder.inner.port_map.is_empty());
        assert!(builder.inner.rlimits.is_empty());
        assert_eq!(builder.inner.workdir_path, None);
        assert_eq!(builder.inner.exec_path, Utf8UnixPathBuf::from("/bin/echo"));
        assert!(builder.inner.args.is_empty());
        assert!(builder.inner.env.is_empty());
        assert_eq!(builder.inner.console_output, None);
        Ok(())
    }
}

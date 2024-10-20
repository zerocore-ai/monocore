use std::path::PathBuf;

use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{PathPair, PortPair, DEFAULT_NUM_VCPUS},
    MonocoreResult,
};

use super::{EnvPair, LinuxRlimit, LogLevel, MicroVM, MicroVMConfig};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The builder for a microVM.
///
/// This struct provides a fluent interface for configuring and creating a `MicroVM` instance.
/// It allows you to set various parameters such as the log level, root path, number of vCPUs,
/// RAM size, virtio-fs mounts, port mappings, resource limits, working directory, executable path,
/// arguments, environment variables, and console output.
///
/// ## Examples
///
/// ```rust
/// use monocore::runtime::{MicroVMBuilder, LogLevel};
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let vm = MicroVMBuilder::default()
///     .log_level(LogLevel::Debug)
///     .root_path("/tmp")
///     .num_vcpus(2)
///     .ram_mib(1024)
///     .virtiofs(["/guest/mount:/host/mount".parse()?])
///     .port_map(["8080:80".parse()?])
///     .rlimits(["RLIMIT_NOFILE=1024:1024".parse()?])
///     .workdir_path("/workdir")
///     .exec_path("/bin/example")
///     .argv(["arg1".to_string(), "arg2".to_string()])
///     .env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?])
///     .console_output("/tmp/console.log")
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct MicroVMBuilder<RootPath, RamMib> {
    log_level: LogLevel,
    root_path: RootPath,
    num_vcpus: Option<u8>,
    ram_mib: RamMib,
    virtiofs: Vec<PathPair>,
    port_map: Vec<PortPair>,
    rlimits: Vec<LinuxRlimit>,
    workdir_path: Option<Utf8UnixPathBuf>,
    exec_path: Option<Utf8UnixPathBuf>,
    argv: Vec<String>,
    env: Vec<EnvPair>,
    console_output: Option<Utf8UnixPathBuf>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<RootPath, RamMib> MicroVMBuilder<RootPath, RamMib> {
    /// Sets the log level for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::{LogLevel, MicroVMBuilder};
    ///
    /// MicroVMBuilder::default().log_level(LogLevel::Debug);
    /// ```
    pub fn log_level(mut self, log_level: LogLevel) -> Self {
        self.log_level = log_level;
        self
    }

    /// Sets the root path for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().root_path("/path/to/root");
    /// ```
    pub fn root_path(self, root_path: impl Into<PathBuf>) -> MicroVMBuilder<PathBuf, RamMib> {
        MicroVMBuilder {
            log_level: self.log_level,
            root_path: root_path.into(),
            num_vcpus: self.num_vcpus,
            ram_mib: self.ram_mib,
            virtiofs: self.virtiofs,
            port_map: self.port_map,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            argv: self.argv,
            env: self.env,
            console_output: self.console_output,
        }
    }

    /// Sets the number of vCPUs for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().num_vcpus(2);
    /// ```
    pub fn num_vcpus(mut self, num_vcpus: u8) -> Self {
        self.num_vcpus = Some(num_vcpus);
        self
    }

    /// Sets the amount of RAM in MiB for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().ram_mib(1024);
    /// ```
    pub fn ram_mib(self, ram_mib: u32) -> MicroVMBuilder<RootPath, u32> {
        MicroVMBuilder {
            log_level: self.log_level,
            root_path: self.root_path,
            num_vcpus: self.num_vcpus,
            ram_mib,
            virtiofs: self.virtiofs,
            port_map: self.port_map,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            argv: self.argv,
            env: self.env,
            console_output: self.console_output,
        }
    }

    /// Sets the virtio-fs mounts for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVMBuilder::default().virtiofs(["/guest/mount:/host/mount".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn virtiofs(mut self, virtiofs: impl IntoIterator<Item = PathPair>) -> Self {
        self.virtiofs = virtiofs.into_iter().collect();
        self
    }

    /// Sets the port map for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVMBuilder::default().port_map(["8080:80".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn port_map(mut self, port_map: impl IntoIterator<Item = PortPair>) -> Self {
        self.port_map = port_map.into_iter().collect();
        self
    }

    /// Sets the resource limits for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVMBuilder::default().rlimits(["RLIMIT_NOFILE=1024:1024".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn rlimits(mut self, rlimits: impl IntoIterator<Item = LinuxRlimit>) -> Self {
        self.rlimits = rlimits.into_iter().collect();
        self
    }

    /// Sets the working directory path for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().workdir_path("/path/to/workdir");
    /// ```
    pub fn workdir_path(mut self, workdir_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.workdir_path = Some(workdir_path.into());
        self
    }

    /// Sets the executable path for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().exec_path("/path/to/exec");
    /// ```
    pub fn exec_path(mut self, exec_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.exec_path = Some(exec_path.into());
        self
    }

    /// Sets the arguments to pass to the executable for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().argv(["arg1".to_string(), "arg2".to_string()]);
    /// ```
    pub fn argv(mut self, argv: impl IntoIterator<Item = String>) -> Self {
        self.argv = argv.into_iter().collect();
        self
    }

    /// Sets the environment variables for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVMBuilder::default().env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn env(mut self, env: impl IntoIterator<Item = EnvPair>) -> Self {
        self.env = env.into_iter().collect();
        self
    }

    /// Sets the console output path for the microVM.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVMBuilder;
    ///
    /// MicroVMBuilder::default().console_output("/tmp/console.log");
    /// ```
    pub fn console_output(mut self, console_output: impl Into<Utf8UnixPathBuf>) -> Self {
        self.console_output = Some(console_output.into());
        self
    }
}

impl MicroVMBuilder<PathBuf, u32> {
    /// Builds the microVM.
    ///
    /// This method creates a `MicroVM` instance based on the configuration set in the builder.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the built `MicroVM` instance if successful, or a `MonocoreError` if there was an error.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # use monocore::runtime::MicroVMBuilder;
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVMBuilder::default()
    ///     .root_path("/tmp")
    ///     .ram_mib(1024)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> MonocoreResult<MicroVM> {
        MicroVM::from_config(MicroVMConfig {
            log_level: self.log_level,
            root_path: self.root_path,
            num_vcpus: self.num_vcpus.unwrap_or(DEFAULT_NUM_VCPUS),
            ram_mib: self.ram_mib,
            virtiofs: self.virtiofs,
            port_map: self.port_map,
            rlimits: self.rlimits,
            workdir_path: self.workdir_path,
            exec_path: self.exec_path,
            argv: self.argv,
            env: self.env,
            console_output: None,
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for MicroVMBuilder<(), ()> {
    fn default() -> Self {
        Self {
            log_level: LogLevel::default(),
            root_path: (),
            num_vcpus: None,
            ram_mib: (),
            virtiofs: vec![],
            port_map: vec![],
            rlimits: vec![],
            workdir_path: None,
            exec_path: None,
            argv: vec![],
            env: vec![],
            console_output: None,
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

        let builder = MicroVMBuilder::default()
            .log_level(LogLevel::Debug)
            .root_path(root_path)
            .num_vcpus(2)
            .ram_mib(1024)
            .virtiofs(["/guest/mount:/host/mount".parse()?])
            .port_map(["8080:80".parse()?])
            .rlimits(["RLIMIT_NOFILE=1024:1024".parse()?])
            .workdir_path(workdir_path)
            .exec_path(exec_path)
            .argv(["arg1".to_string(), "arg2".to_string()])
            .env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?])
            .console_output("/tmp/console.log");

        assert_eq!(builder.log_level, LogLevel::Debug);
        assert_eq!(builder.root_path, PathBuf::from(root_path));
        assert_eq!(builder.num_vcpus, Some(2));
        assert_eq!(builder.ram_mib, 1024);
        assert_eq!(builder.virtiofs, ["/guest/mount:/host/mount".parse()?]);
        assert_eq!(builder.port_map, ["8080:80".parse()?]);
        assert_eq!(builder.rlimits, ["RLIMIT_NOFILE=1024:1024".parse()?]);
        assert_eq!(
            builder.workdir_path,
            Some(Utf8UnixPathBuf::from(workdir_path))
        );
        assert_eq!(builder.exec_path, Some(Utf8UnixPathBuf::from(exec_path)));
        assert_eq!(builder.argv, ["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(
            builder.env,
            ["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?]
        );
        assert_eq!(
            builder.console_output,
            Some(Utf8UnixPathBuf::from("/tmp/console.log"))
        );
        Ok(())
    }

    #[test]
    fn test_microvm_builder_minimal() -> anyhow::Result<()> {
        let root_path = "/tmp";
        let ram_mib = 512;

        let builder = MicroVMBuilder::default()
            .root_path(root_path)
            // .exec_path("/bin/sh")
            .ram_mib(ram_mib);

        assert_eq!(builder.root_path, PathBuf::from(root_path));
        assert_eq!(builder.ram_mib, ram_mib);

        // // Check that other fields have default values
        // assert_eq!(builder.log_level, LogLevel::default());
        // assert_eq!(builder.num_vcpus, None);
        // assert!(builder.virtiofs.is_empty());
        // assert!(builder.port_map.is_empty());
        // assert!(builder.rlimits.is_empty());
        // assert_eq!(builder.workdir_path, None);
        // assert_eq!(builder.exec_path, None);
        // assert!(builder.argv.is_empty());
        // assert!(builder.env.is_empty());
        // assert_eq!(builder.console_output, None);

        // Attempt to build the MicroVM
        let vm = builder.build();

        println!("vm = {:?}", vm);

        // Check that the VM was created successfully
        assert!(vm.is_ok());

        Ok(())
    }
}

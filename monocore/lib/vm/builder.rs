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
pub struct MicroVmConfigBuilder<RootPath, RamMib> {
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
/// use monocore::runtime::{MicroVmBuilder, LogLevel};
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let vm = MicroVmBuilder::default()
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
pub struct MicroVmBuilder<RootPath, RamMib> {
    inner: MicroVmConfigBuilder<RootPath, RamMib>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<RootPath, RamMib> MicroVmConfigBuilder<RootPath, RamMib> {
    /// Sets the log level for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::{LogLevel, MicroVmConfigBuilder};
    ///
    /// MicroVmConfigBuilder::default().log_level(LogLevel::Debug);
    /// ```
    pub fn log_level(mut self, log_level: LogLevel) -> Self {
        self.log_level = log_level;
        self
    }

    /// Sets the root path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().root_path("/path/to/root");
    /// ```
    pub fn root_path(self, root_path: impl Into<PathBuf>) -> MicroVmConfigBuilder<PathBuf, RamMib> {
        MicroVmConfigBuilder {
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

    /// Sets the number of vCPUs for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().num_vcpus(2);
    /// ```
    pub fn num_vcpus(mut self, num_vcpus: u8) -> Self {
        self.num_vcpus = Some(num_vcpus);
        self
    }

    /// Sets the amount of RAM in MiB for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().ram_mib(1024);
    /// ```
    pub fn ram_mib(self, ram_mib: u32) -> MicroVmConfigBuilder<RootPath, u32> {
        MicroVmConfigBuilder {
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

    /// Sets the virtio-fs mounts for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmConfigBuilder::default().virtiofs(["/guest/mount:/host/mount".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn virtiofs(mut self, virtiofs: impl IntoIterator<Item = PathPair>) -> Self {
        self.virtiofs = virtiofs.into_iter().collect();
        self
    }

    /// Sets the port map for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmConfigBuilder::default().port_map(["8080:80".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn port_map(mut self, port_map: impl IntoIterator<Item = PortPair>) -> Self {
        self.port_map = port_map.into_iter().collect();
        self
    }

    /// Sets the resource limits for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmConfigBuilder::default().rlimits(["RLIMIT_NOFILE=1024:1024".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn rlimits(mut self, rlimits: impl IntoIterator<Item = LinuxRlimit>) -> Self {
        self.rlimits = rlimits.into_iter().collect();
        self
    }

    /// Sets the working directory path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().workdir_path("/path/to/workdir");
    /// ```
    pub fn workdir_path(mut self, workdir_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.workdir_path = Some(workdir_path.into());
        self
    }

    /// Sets the executable path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().exec_path("/path/to/exec");
    /// ```
    pub fn exec_path(mut self, exec_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.exec_path = Some(exec_path.into());
        self
    }

    /// Sets the arguments to pass to the executable for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().argv(["arg1".to_string(), "arg2".to_string()]);
    /// ```
    pub fn argv(mut self, argv: impl IntoIterator<Item = String>) -> Self {
        self.argv = argv.into_iter().collect();
        self
    }

    /// Sets the environment variables for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmConfigBuilder::default().env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn env(mut self, env: impl IntoIterator<Item = EnvPair>) -> Self {
        self.env = env.into_iter().collect();
        self
    }

    /// Sets the console output path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmConfigBuilder;
    ///
    /// MicroVmConfigBuilder::default().console_output("/tmp/console.log");
    /// ```
    pub fn console_output(mut self, console_output: impl Into<Utf8UnixPathBuf>) -> Self {
        self.console_output = Some(console_output.into());
        self
    }
}

impl<RootPath, RamMib> MicroVmBuilder<RootPath, RamMib> {
    /// Sets the log level for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::{LogLevel, MicroVmBuilder};
    ///
    /// MicroVmBuilder::default().log_level(LogLevel::Debug);
    /// ```
    pub fn log_level(mut self, log_level: LogLevel) -> Self {
        self.inner = self.inner.log_level(log_level);
        self
    }

    /// Sets the root path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().root_path("/path/to/root");
    /// ```
    pub fn root_path(self, root_path: impl Into<PathBuf>) -> MicroVmBuilder<PathBuf, RamMib> {
        MicroVmBuilder {
            inner: self.inner.root_path(root_path),
        }
    }

    /// Sets the number of vCPUs for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().num_vcpus(2);
    /// ```
    pub fn num_vcpus(mut self, num_vcpus: u8) -> Self {
        self.inner = self.inner.num_vcpus(num_vcpus);
        self
    }

    /// Sets the amount of RAM in MiB for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().ram_mib(1024);
    /// ```
    pub fn ram_mib(self, ram_mib: u32) -> MicroVmBuilder<RootPath, u32> {
        MicroVmBuilder {
            inner: self.inner.ram_mib(ram_mib),
        }
    }

    /// Sets the virtio-fs mounts for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmBuilder::default().virtiofs(["/guest/mount:/host/mount".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn virtiofs(mut self, virtiofs: impl IntoIterator<Item = PathPair>) -> Self {
        self.inner = self.inner.virtiofs(virtiofs);
        self
    }

    /// Sets the port map for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmBuilder::default().port_map(["8080:80".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn port_map(mut self, port_map: impl IntoIterator<Item = PortPair>) -> Self {
        self.inner = self.inner.port_map(port_map);
        self
    }

    /// Sets the resource limits for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
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
    /// use monocore::runtime::MicroVmBuilder;
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
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().exec_path("/path/to/exec");
    /// ```
    pub fn exec_path(mut self, exec_path: impl Into<Utf8UnixPathBuf>) -> Self {
        self.inner = self.inner.exec_path(exec_path);
        self
    }

    /// Sets the arguments to pass to the executable for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().argv(["arg1".to_string(), "arg2".to_string()]);
    /// ```
    pub fn argv(mut self, argv: impl IntoIterator<Item = String>) -> Self {
        self.inner = self.inner.argv(argv);
        self
    }

    /// Sets the environment variables for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// MicroVmBuilder::default().env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn env(mut self, env: impl IntoIterator<Item = EnvPair>) -> Self {
        self.inner = self.inner.env(env);
        self
    }

    /// Sets the console output path for the MicroVm.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::runtime::MicroVmBuilder;
    ///
    /// MicroVmBuilder::default().console_output("/tmp/console.log");
    /// ```
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
}

impl MicroVmBuilder<PathBuf, u32> {
    /// Builds the MicroVm.
    ///
    /// This method creates a `MicroVm` instance based on the configuration set in the builder.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the built `MicroVm` instance if successful, or a `MonocoreError` if there was an error.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// # use monocore::runtime::MicroVmConfigBuilder;
    /// # fn main() -> anyhow::Result<()> {
    /// let vm = MicroVmConfigBuilder::default()
    ///     .root_path("/tmp")
    ///     .ram_mib(1024)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> MonocoreResult<MicroVm> {
        MicroVm::from_config(MicroVmConfig {
            log_level: self.inner.log_level,
            root_path: self.inner.root_path,
            num_vcpus: self.inner.num_vcpus.unwrap_or(DEFAULT_NUM_VCPUS),
            ram_mib: self.inner.ram_mib,
            virtiofs: self.inner.virtiofs,
            port_map: self.inner.port_map,
            rlimits: self.inner.rlimits,
            workdir_path: self.inner.workdir_path,
            exec_path: self.inner.exec_path,
            argv: self.inner.argv,
            env: self.inner.env,
            console_output: None,
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
            .virtiofs(["/guest/mount:/host/mount".parse()?])
            .port_map(["8080:80".parse()?])
            .rlimits(["RLIMIT_NOFILE=1024:1024".parse()?])
            .workdir_path(workdir_path)
            .exec_path(exec_path)
            .argv(["arg1".to_string(), "arg2".to_string()])
            .env(["KEY1=VALUE1".parse()?, "KEY2=VALUE2".parse()?])
            .console_output("/tmp/console.log");

        assert_eq!(builder.inner.log_level, LogLevel::Debug);
        assert_eq!(builder.inner.root_path, PathBuf::from(root_path));
        assert_eq!(builder.inner.num_vcpus, Some(2));
        assert_eq!(builder.inner.ram_mib, 1024);
        assert_eq!(
            builder.inner.virtiofs,
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
        assert_eq!(builder.inner.argv, ["arg1".to_string(), "arg2".to_string()]);
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
        assert_eq!(builder.inner.num_vcpus, None);
        assert!(builder.inner.virtiofs.is_empty());
        assert!(builder.inner.port_map.is_empty());
        assert!(builder.inner.rlimits.is_empty());
        assert_eq!(builder.inner.workdir_path, None);
        assert_eq!(builder.inner.exec_path, None);
        assert!(builder.inner.argv.is_empty());
        assert!(builder.inner.env.is_empty());
        assert_eq!(builder.inner.console_output, None);

        // Attempt to build the MicroVm
        let vm = builder.build();

        println!("vm = {:?}", vm);

        // Check that the VM was created successfully
        assert!(vm.is_ok());

        Ok(())
    }
}

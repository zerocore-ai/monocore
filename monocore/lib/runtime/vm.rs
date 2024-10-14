use std::path::PathBuf;

use getset::Getters;
use typed_path::UnixPathBuf;

use crate::config::PathPair;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A configuration for a microVM.
#[derive(Debug, Getters)]
pub struct VMConfig {
    /// The log level to use for the microVM.
    log_level: Option<u32>,

    /// The path to the root directory for the microVM.
    root_path: Option<PathBuf>,

    /// The number of vCPUs to use for the microVM.
    num_vcpus: Option<u8>,

    /// The amount of RAM in MiB to use for the microVM.
    ram_mib: Option<u32>,

    /// The virtio-fs mounts to use for the microVM.
    virtiofs: Vec<PathPair>,

    /// The port map to use for the microVM.
    port_map: Vec<String>,

    /// The resource limits to use for the microVM.
    rlimits: Vec<String>,

    /// The working directory path to use for the microVM.
    workdir_path: Option<PathBuf>,

    /// The executable path to use for the microVM.
    exec_path: Option<UnixPathBuf>,

    /// The arguments to pass to the executable.
    argv: Vec<String>,

    /// The environment variables to set for the executable.
    envp: Vec<String>,

    /// The console output path to use for the microVM.
    console_output: Option<PathBuf>,
}

/// A microVM.
#[derive(Debug)]
pub struct VM {
    ctx_id: u32,
    config: VMConfig,
}

/// A builder for a microVM.
pub struct VMBuilder {
    config: VMConfig,
}

use std::{net::IpAddr, path::PathBuf};

use clap::{Parser, Subcommand};

use crate::cli::styles;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Arguments for the mcrun command
#[derive(Debug, Parser)]
#[command(name = "mcrun", author, styles=styles::styles())]
pub struct McrunArgs {
    /// The subcommand to run
    #[command(subcommand)]
    pub subcommand: McrunSubcommand,
}

/// Available subcommands for managing microvms
#[derive(Subcommand, Debug)]
pub enum McrunSubcommand {
    /// Run as microvm
    Microvm {
        /// Log level
        #[arg(long)]
        log_level: Option<u8>,

        /// Root filesystem path
        #[arg(long)]
        root_path: PathBuf,

        /// Number of virtual CPUs
        #[arg(long)]
        num_vcpus: u8,

        /// RAM size in MiB
        #[arg(long)]
        ram_mib: u32,

        /// Working directory path
        #[arg(long)]
        workdir_path: Option<String>,

        /// Executable path
        #[arg(long)]
        exec_path: String,

        /// Environment variables (KEY=VALUE format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        env: Vec<String>,

        /// Directory mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        mapped_dirs: Vec<String>,

        /// Port mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        port_map: Vec<String>,

        /// Additional arguments after `--`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Run as supervisor
    Supervisor {
        /// Directory for log files
        #[arg(long)]
        log_dir: PathBuf,

        /// Name of the child process
        #[arg(long)]
        child_name: String,

        /// Path to the sandbox metrics and metadata database file
        #[arg(long)]
        sandbox_db_path: PathBuf,

        /// Whether to forward output to stdout/stderr
        #[arg(long, default_value = "true")]
        forward_output: bool,

        /// The paths to the overlayfs layers
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        overlayfs_layer_paths: Vec<PathBuf>,

        // File specific arguments
        /// The host to bind to
        #[arg(long)]
        nfs_host: Option<IpAddr>,

        /// The port to bind to
        #[arg(long)]
        nfs_port: Option<u16>,

        // Sandbox specific arguments
        /// Root filesystem path
        #[arg(long)]
        root_path: PathBuf,

        /// Number of virtual CPUs
        #[arg(long)]
        num_vcpus: u8,

        /// RAM size in MiB
        #[arg(long)]
        ram_mib: u32,

        /// Working directory path
        #[arg(long)]
        workdir_path: String,

        /// Executable path
        #[arg(long)]
        exec_path: String,

        /// Environment variables (KEY=VALUE format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        env: Vec<String>,

        /// Directory mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        mapped_dirs: Vec<String>,

        /// Port mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        port_map: Vec<String>,

        /// Log level
        #[arg(long)]
        log_level: Option<u8>,

        /// Additional arguments after `--`
        #[arg(last = true)]
        args: Vec<String>,
    },
}

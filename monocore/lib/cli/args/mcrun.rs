use std::path::PathBuf;

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

        /// Arguments for the executable
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        args: Vec<String>,

        /// Environment variables (KEY=VALUE format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        env: Vec<String>,

        /// Directory mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        mapped_dirs: Vec<String>,

        /// Port mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        port_map: Vec<String>,
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

        /// Log level
        #[arg(long)]
        log_level: Option<u8>,

        /// Whether to forward output to stdout/stderr
        #[arg(long, default_value = "true")]
        forward_output: bool,

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
        workdir_path: Option<String>,

        /// Executable path
        #[arg(long)]
        exec_path: String,

        /// Arguments for the executable
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        args: Vec<String>,

        /// Environment variables (KEY=VALUE format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        env: Vec<String>,

        /// Directory mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        mapped_dirs: Vec<String>,

        /// Port mappings (host:guest format)
        #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
        port_map: Vec<String>,
    },
}

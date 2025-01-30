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
        /// Root filesystem path
        #[arg(long)]
        root_path: PathBuf,

        /// Number of virtual CPUs
        #[arg(long)]
        num_vcpus: u8,

        /// RAM size in MiB
        #[arg(long)]
        ram_mib: u32,

        /// Directory mappings (host:guest format)
        #[arg(long)]
        mapped_dirs: Vec<String>,

        /// Port mappings (host:guest format)
        #[arg(long)]
        port_map: Vec<String>,

        /// Working directory path
        #[arg(long)]
        workdir_path: Option<String>,

        /// Executable path
        #[arg(long)]
        exec_path: String,

        /// Arguments for the executable
        #[arg(long)]
        args: Vec<String>,

        /// Environment variables (KEY=VALUE format)
        #[arg(long)]
        env: Vec<String>,
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
    },
}

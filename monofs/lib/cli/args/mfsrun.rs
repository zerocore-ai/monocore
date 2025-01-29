use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::{
    cli::styles,
    config::{DEFAULT_HOST, DEFAULT_NFS_PORT},
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Arguments for the mfsrun command
#[derive(Debug, Parser)]
#[command(name = "monofs", author, styles=styles::styles())]
pub struct MfsRuntimeArgs {
    /// The subcommand to run
    #[command(subcommand)]
    pub subcommand: MfsRuntimeSubcommand,
}

/// Available subcommands for managing services
#[derive(Subcommand, Debug)]
pub enum MfsRuntimeSubcommand {
    /// Run as NFS server
    Nfsserver {
        /// Host address to bind to
        #[arg(long, default_value = DEFAULT_HOST)]
        host: String,

        /// Port to listen on
        #[arg(long, default_value_t = DEFAULT_NFS_PORT)]
        port: u32,

        /// The directory to store the filesystem data
        #[arg(long)]
        store_dir: PathBuf,
    },
    /// Run as supervisor
    Supervisor {
        /// Directory for log files
        #[arg(long)]
        log_dir: PathBuf,

        /// Name of the child process
        #[arg(long)]
        child_name: String,

        /// Host address for NFS server to bind to
        #[arg(long, default_value = DEFAULT_HOST)]
        host: String,

        /// Port for NFS server to listen on
        #[arg(long, default_value_t = DEFAULT_NFS_PORT)]
        port: u32,

        /// The directory to store the filesystem data
        #[arg(long)]
        store_dir: PathBuf,

        /// Path to the metrics database file
        #[arg(long)]
        db_path: PathBuf,

        /// Directory where the filesystem is mounted
        #[arg(long)]
        mount_dir: PathBuf,
    },
}

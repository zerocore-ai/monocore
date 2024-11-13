use std::path::PathBuf;

use clap::Parser;
use tracing::Level;

use crate::utils::{MONOCORE_OCI_DIR, MONOCORE_ROOTFS_DIR};

use super::styles;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// Monocore CLI - A lightweight orchestrator for running containers in microVMs
#[derive(Debug, Parser)]
#[command(name = "monocore", author, about, version, styles=styles::styles())]
pub struct MonocoreArgs {
    /// The subcommand to run
    #[command(subcommand)]
    pub subcommand: Option<MonocoreSubcommand>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

/// Available subcommands for managing services
#[derive(Debug, Parser)]
pub enum MonocoreSubcommand {
    /// Start services defined in config file. If group specified, only starts services in that group
    #[command(arg_required_else_help = true)]
    Up {
        /// Config file path (default: monocore.toml)
        #[arg(short, long, default_value = "monocore.toml")]
        file: PathBuf,

        /// Only start services in this group
        #[arg(short, long)]
        group: Option<String>,

        /// Directory for OCI images
        #[arg(long, default_value = MONOCORE_OCI_DIR.as_os_str())]
        oci_dir: PathBuf,

        /// Directory for merged root filesystems
        #[arg(long, default_value = MONOCORE_ROOTFS_DIR.as_os_str())]
        rootfs_dir: PathBuf,
    },

    /// Stop running services. If group specified, only stops services in that group
    Down {
        /// Config file path
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Only stop services in this group
        #[arg(short, long)]
        group: Option<String>,

        /// Directory containing service root filesystems
        #[arg(long, default_value = MONOCORE_ROOTFS_DIR.as_os_str())]
        rootfs_dir: PathBuf,
    },

    /// Pull container image from registry
    #[command(arg_required_else_help = true)]
    Pull {
        /// Image reference (e.g. 'ubuntu:22.04')
        #[arg(required = true)]
        image: String,

        /// Directory for OCI images
        #[arg(long, default_value = MONOCORE_OCI_DIR.as_os_str())]
        oci_dir: PathBuf,
    },

    /// Show status of running services (CPU, memory, network, etc)
    Status {},
}

//-------------------------------------------------------------------------------------------------
// Methods
//-------------------------------------------------------------------------------------------------

impl MonocoreArgs {
    /// Initialize logging system with INFO or DEBUG level based on verbose flag
    pub fn init_logging(&self) {
        let level = if self.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        };

        tracing_subscriber::fmt().with_max_level(level).init();
    }
}

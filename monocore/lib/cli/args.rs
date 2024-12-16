use std::path::PathBuf;

use clap::Parser;
use tracing::Level;

use crate::config::{DEFAULT_MONOCORE_HOME, DEFAULT_SERVER_PORT};

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

        /// Home directory for monocore state (default: ~/.monocore)
        #[arg(long, default_value = DEFAULT_MONOCORE_HOME.as_os_str())]
        home_dir: PathBuf,
    },

    /// Stop running services. If group specified, only stops services in that group
    Down {
        /// Config file path
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Only stop services in this group
        #[arg(short, long)]
        group: Option<String>,

        /// Home directory for monocore state (default: ~/.monocore)
        #[arg(long, default_value = DEFAULT_MONOCORE_HOME.as_os_str())]
        home_dir: PathBuf,
    },

    /// Pull container image from the Docker registry
    #[command(arg_required_else_help = true)]
    Pull {
        /// Image reference (e.g. 'alpine:latest')
        #[arg(required = true)]
        image: String,

        /// Home directory for monocore state (default: ~/.monocore)
        #[arg(long, default_value = DEFAULT_MONOCORE_HOME.as_os_str())]
        home_dir: PathBuf,
    },

    /// Show status of running services (CPU, memory, network, etc)
    Status {},

    /// Display service logs
    #[command(arg_required_else_help = false)]
    Log {
        /// Name of the service to show logs for. If not specified, shows supervisor logs
        service: Option<String>,

        /// Number of lines to show (from the end)
        #[arg(short = 'n')]
        lines: Option<usize>,

        /// Disable pager and print directly to stdout
        #[arg(long)]
        no_pager: bool,

        /// Follow log output (like tail -f)
        #[arg(short = 'f', long)]
        follow: bool,

        /// Home directory for monocore state (default: ~/.monocore)
        #[arg(long, default_value = DEFAULT_MONOCORE_HOME.as_os_str())]
        home_dir: PathBuf,
    },

    /// Remove service files (rootfs and config)
    #[command(alias = "rm", arg_required_else_help = true)]
    Remove {
        /// Names of services to remove
        #[arg(required_unless_present = "group")]
        services: Vec<String>,

        /// Remove all services in this group
        #[arg(short, long)]
        group: Option<String>,

        /// Home directory for monocore state (default: ~/.monocore)
        #[arg(long, default_value = DEFAULT_MONOCORE_HOME.as_os_str())]
        home_dir: PathBuf,
    },

    /// Start monocore in server mode
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value_t = std::env::var("PORT").unwrap_or(DEFAULT_SERVER_PORT.to_string()).parse().unwrap())]
        port: u16,

        /// Home directory for monocore state (default: ~/.monocore)
        #[arg(long, default_value = DEFAULT_MONOCORE_HOME.as_os_str())]
        home_dir: PathBuf,
    },
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

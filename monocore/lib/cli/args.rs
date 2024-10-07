use std::path::PathBuf;

use clap::Parser;

use super::styles;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// Monocore CLI arguments.
#[derive(Debug, Parser)]
#[command(name = "mono", author, about, version, styles=styles::styles())]
pub struct MonocoreArgs {
    /// The subcommand to run.
    #[command(subcommand)]
    pub subcommand: Option<MonocoreSubcommand>,
}

/// The subcommands of the Monocore CLI.
#[derive(Debug, Parser)]
pub enum MonocoreSubcommand {
    /// Starts the specified Monocore service or services.
    Up {
        /// The path to the configuration file to use.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// The orchestration group the services should be started in.
        #[arg(short, long)]
        group: Option<String>,
    },

    /// Stops the specified Monocore service or services.
    Down {
        /// The path to the configuration file to use.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// The orchestration group the services should be stopped in.
        #[arg(short, long)]
        group: Option<String>,
    },

    /// Pushes an image to the registry.
    Push {},

    /// Pulls an image from the registry.
    Pull {
        /// The image to pull.
        #[arg()]
        image: String,
    },

    /// Runs specified script command.
    Run {},

    /// Prints the status of the specified Monocore service or services.
    Status {},
}

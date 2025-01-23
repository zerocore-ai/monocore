use std::path::PathBuf;

use super::styles;
use clap::Parser;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// monofs is a tool for managing distributed filesystems
#[derive(Debug, Parser)]
#[command(name = "monofs", author, styles=styles::styles())]
pub struct MonofsArgs {
    /// The subcommand to run
    #[command(subcommand)]
    pub subcommand: Option<MonofsSubcommand>,

    /// Enable verbose logging
    #[arg(short = 'V', long)]
    pub verbose: bool,

    /// Show version
    #[arg(short = 'v', long)]
    pub version: bool,
}

/// Available subcommands for managing services
#[derive(Debug, Parser)]
pub enum MonofsSubcommand {
    /// Initialize a new monofs filesystem
    #[command(name = "init")]
    Init {
        /// System path where the filesystem should be initialized
        #[arg()]
        system_path: Option<PathBuf>,
    },

    /// Create a temporary filesystem
    #[command(name = "tmp")]
    Tmp,

    /// Clone an existing filesystem
    #[command(name = "clone")]
    Clone {
        /// Remote or system path to clone from
        #[arg()]
        remote_or_system_path: String,
    },

    /// Sync a filesystem with another filesystem
    #[command(name = "sync")]
    Sync {
        /// Remote or system path to sync with
        #[arg()]
        remote_or_system_path: String,

        /// Type of sync (e.g. backup, raft, crdt)
        #[arg(short = 't', long)]
        sync_type: String,
    },

    /// Show the revisions of a filesystem
    #[command(name = "rev")]
    Rev {
        /// Path to show revisions for
        #[arg(short = 'p', long)]
        path: Option<String>,
    },

    /// Tag a revision of a file entity
    #[command(name = "tag")]
    Tag {
        /// Revision to tag
        #[arg()]
        revision: String,

        /// Tag name
        #[arg()]
        tag: String,

        /// Path to tag
        #[arg(short = 'p', long)]
        path: Option<String>,
    },

    /// Checkout a revision of a file entity
    #[command(name = "checkout")]
    Checkout {
        /// Revision to checkout
        #[arg()]
        revision: String,

        /// Path to checkout
        #[arg(short = 'p', long)]
        path: Option<String>,
    },

    /// Show differences between two revisions of a file entity
    #[command(name = "diff")]
    Diff {
        /// First revision to compare
        #[arg()]
        revision1: String,

        /// Second revision to compare
        #[arg()]
        revision2: String,

        /// Path to compare
        #[arg(short = 'p', long)]
        path: Option<String>,
    },

    /// Show version information
    #[command(name = "version")]
    Version,
}

//-------------------------------------------------------------------------------------------------
// Methods
//-------------------------------------------------------------------------------------------------

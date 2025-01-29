use std::path::PathBuf;

use crate::cli::styles;
use clap::Parser;
use typed_path::Utf8UnixPathBuf;

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
        /// Directory where the filesystem will be mounted
        mount_dir: Option<PathBuf>,
    },

    /// Create a temporary filesystem
    #[command(name = "tmp")]
    Tmp,

    /// Clone an existing filesystem
    #[command(name = "clone")]
    Clone {
        /// Remote or local path to clone from
        uri: String,
    },

    /// Sync a filesystem with another filesystem
    #[command(name = "sync")]
    Sync {
        /// Remote or local path to sync with
        uri: String,

        /// Type of sync (e.g. backup, raft, crdt)
        #[arg(short = 't', long)]
        r#type: String,
    },

    /// Show the revisions of a filesystem
    #[command(name = "rev")]
    Rev {
        /// Path to show revisions for
        #[arg(short = 'p', long)]
        path: Option<Utf8UnixPathBuf>,
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
        path: Option<Utf8UnixPathBuf>,
    },

    /// Checkout a revision of a file entity
    #[command(name = "checkout")]
    Checkout {
        /// Revision to checkout
        #[arg()]
        revision: String,

        /// Path to checkout
        #[arg(short = 'p', long)]
        path: Option<Utf8UnixPathBuf>,
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
        path: Option<Utf8UnixPathBuf>,
    },

    /// Safely unmount the filesystem and stop the NFS server
    #[command(name = "detach")]
    Detach {
        /// Directory where the filesystem is mounted
        mount_dir: Option<PathBuf>,

        /// Force unmount even if busy
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Show version information
    #[command(name = "version")]
    Version,
}

//-------------------------------------------------------------------------------------------------
// Methods
//-------------------------------------------------------------------------------------------------

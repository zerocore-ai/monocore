use std::path::PathBuf;

use crate::{cli::styles, oci::Reference};
use clap::Parser;
use typed_path::Utf8UnixPathBuf;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// `monocore` is a tool for managing lightweight virtual machines and images
#[derive(Debug, Parser)]
#[command(name = "monocore", author, styles=styles::styles())]
pub struct MonocoreArgs {
    /// The subcommand to run
    #[command(subcommand)]
    pub subcommand: Option<MonocoreSubcommand>,

    /// Enable verbose logging
    #[arg(short = 'V', long)]
    pub verbose: bool,

    /// Show version
    #[arg(short = 'v', long)]
    pub version: bool,
}

/// Available subcommands for managing services
#[derive(Debug, Parser)]
pub enum MonocoreSubcommand {
    /// Initialize a new monocore project
    #[command(name = "init")]
    Init {
        /// Specifies the directory to initialize the project in
        #[arg(required = false, name = "PATH")]
        path: Option<PathBuf>,

        /// Specifies the directory to initialize the project in
        #[arg(short, long = "path", name = "PATH\0")]
        path_with_flag: Option<PathBuf>,
    },

    /// Add a new build, sandbox, or group component to the project
    #[command(name = "add")]
    Add {
        /// Add a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Add a build
        #[arg(short, long)]
        build: bool,

        /// Add a group
        #[arg(short, long)]
        group: bool,

        /// Names of components to add
        #[arg(required = true)]
        names: Vec<String>,

        /// Image to use
        #[arg(short, long)]
        image: Option<String>,

        /// Number of CPUs
        #[arg(long)]
        cpus: Option<u32>,

        /// RAM in MB
        #[arg(long)]
        ram: Option<u32>,

        /// Volume mappings
        #[arg(long)]
        volumes: Vec<String>,

        /// Port mappings
        #[arg(long)]
        ports: Vec<String>,

        /// Environment variables
        #[arg(long)]
        envs: Vec<String>,

        /// Groups to join
        #[arg(long)]
        groups: Vec<String>,

        /// Working directory
        #[arg(long)]
        workdir: Option<Utf8UnixPathBuf>,

        /// Shell to use
        #[arg(long)]
        shell: Option<String>,

        /// Scripts to add
        #[arg(long)]
        scripts: Vec<String>,

        /// Files to import
        #[arg(long)]
        imports: Vec<String>,

        /// Files to export
        #[arg(long)]
        exports: Vec<String>,

        /// Network configuration
        #[arg(long)]
        network: Option<String>,
    },

    /// Remove a build, sandbox, or group component from the project
    #[command(name = "remove")]
    Remove {
        /// Remove a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Remove a build
        #[arg(short, long)]
        build: bool,

        /// Remove a group
        #[arg(short, long)]
        group: bool,

        /// Names of components to remove
        #[arg(required = true)]
        names: Vec<String>,
    },

    /// List build, sandbox, or group components in the project
    #[command(name = "list")]
    List {
        /// List sandboxes
        #[arg(short, long)]
        sandbox: bool,

        /// List builds
        #[arg(short, long)]
        build: bool,

        /// List groups
        #[arg(short, long)]
        group: bool,
    },

    /// Show logs of a running build, sandbox, or group
    #[command(name = "log")]
    Log {
        /// Show logs of a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Show logs of a build
        #[arg(short, long)]
        build: bool,

        /// Show logs of a group
        #[arg(short, long)]
        group: bool,

        /// Name of the component
        #[arg(required = true)]
        name: String,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,

        /// Follow the logs
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show from the end
        #[arg(short = 'n', long)]
        tail: Option<usize>,
    },

    /// Show tree of layers that make up a build, sandbox, or group component
    #[command(name = "tree")]
    Tree {
        /// Show sandbox tree
        #[arg(short, long)]
        sandbox: bool,

        /// Show build tree
        #[arg(short, long)]
        build: bool,

        /// Show group tree
        #[arg(short, long)]
        group: bool,

        /// Names of components to show
        #[arg(required = true)]
        names: Vec<String>,

        /// Maximum depth level
        #[arg(short = 'L')]
        level: Option<usize>,
    },

    /// Run a sandbox script
    #[command(name = "run")]
    Run {
        /// Run a sandbox script
        #[arg(short, long)]
        sandbox: bool,

        /// Run a build script
        #[arg(short, long)]
        build: bool,

        /// Name of the component
        #[arg(required = true, name = "NAME[~SCRIPT]")]
        name: String,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,

        /// Additional arguments after `--`
        #[arg(last = true)]
        args: Vec<String>,

        /// Run sandbox in the background
        #[arg(short, long)]
        detach: bool,

        /// Execute a command within the sandbox
        #[arg(short, long)]
        exec: Option<String>,
    },

    /// Start a sandbox
    #[command(name = "start")]
    Start {
        /// Run start command for a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Run start command for a build
        #[arg(short, long)]
        build: bool,

        /// Name of the component
        #[arg(required = true)]
        name: String,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,

        /// Additional arguments
        #[arg(last = true)]
        args: Vec<String>,

        /// Run sandbox in the background
        #[arg(short, long)]
        detach: bool,
    },

    /// Open a shell in a sandbox
    #[command(name = "shell")]
    Shell {
        /// Open a shell in a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Open a shell in a build
        #[arg(short, long)]
        build: bool,

        /// Name of the component
        #[arg(required = true)]
        name: String,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,

        /// Additional arguments
        #[arg(last = true)]
        args: Vec<String>,

        /// Run sandbox in the background
        #[arg(short, long)]
        detach: bool,
    },

    /// Create a temporary sandbox
    #[command(name = "tmp")]
    Tmp {
        /// Create a temporary sandbox from an image
        #[arg(short, long)]
        image: bool,

        /// Name of the image
        #[arg(required = true, name = "NAME[~SCRIPT]")]
        name: String,

        /// Number of CPUs
        #[arg(long)]
        cpus: Option<u8>,

        /// RAM in MB
        #[arg(long)]
        ram: Option<u32>,

        /// Volume mappings
        #[arg(long)]
        volumes: Vec<String>,

        /// Port mappings
        #[arg(long)]
        ports: Vec<String>,

        /// Environment variables
        #[arg(long)]
        envs: Vec<String>,

        /// Working directory
        #[arg(long)]
        workdir: Option<Utf8UnixPathBuf>,

        /// Execute a command within the sandbox
        #[arg(short, long)]
        exec: Option<String>,

        /// Additional arguments after `--`
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Install a script from an image
    #[command(name = "install")]
    Install {
        /// Whether to install from an image
        #[arg(short, long)]
        image: bool,

        /// Whether to install from an image group
        #[arg(short = 'G', long)]
        image_group: bool,

        /// Name of the image or image group
        #[arg(required = true)]
        name: String,

        /// Script to install
        script: Option<String>,

        /// New name for the script
        rename: Option<String>,
    },

    /// Uninstall a script
    #[command(name = "uninstall")]
    Uninstall {
        /// Script to uninstall
        script: Option<String>,

        /// Whether to uninstall from an image
        #[arg(short, long)]
        image: bool,

        /// Whether to uninstall from an image group
        #[arg(short = 'G', long)]
        image_group: bool,

        /// Name of the image or image group
        #[arg(required = true)]
        name: String,
    },

    /// Start or stop project sandboxes based on configuration
    #[command(name = "apply")]
    Apply {
        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Start project sandboxes
    #[command(name = "up")]
    Up {
        /// Whether to start a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to start a build
        #[arg(short, long)]
        build: bool,

        /// Whether to start a group
        #[arg(short, long)]
        group: bool,

        /// Names of components to start
        #[arg(required = true)]
        names: Vec<String>,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Stop project sandboxes
    #[command(name = "down")]
    Down {
        /// Whether to stop a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to stop a build
        #[arg(short, long)]
        build: bool,

        /// Whether to stop a group
        #[arg(short, long)]
        group: bool,

        /// Names of components to stop
        #[arg(required = true)]
        names: Vec<String>,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Show running status
    #[command(name = "status")]
    Status {
        /// Whether to show a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to show a build
        #[arg(short, long)]
        build: bool,

        /// Whether to show a group
        #[arg(short, long)]
        group: bool,

        /// Name of the component
        #[arg(required = true)]
        name: String,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Clean project data
    #[command(name = "clean")]
    Clean,

    /// Build images
    #[command(name = "build")]
    Build {
        /// Build from build definition
        #[arg(short, long)]
        build: bool,

        /// Build from sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Build from group
        #[arg(short, long)]
        group: bool,

        /// Name of the component
        #[arg(required = true)]
        name: String,

        /// Create a snapshot
        #[arg(long)]
        snapshot: bool,
    },

    /// Pull an image
    #[command(name = "pull")]
    Pull {
        /// Whether to pull an image
        #[arg(short, long)]
        image: bool,

        /// Whether to pull an image group
        #[arg(short = 'G', long)]
        image_group: bool,

        /// Name of the image or image group
        #[arg(required = true)]
        name: Reference,

        /// Path to store the layer files
        #[arg(short = 'L', long)]
        layer_path: Option<PathBuf>,
    },

    /// Push an image
    #[command(name = "push")]
    Push {
        /// Image to push
        #[arg(short, long)]
        image: String,
    },

    /// Manage monocore itself
    #[command(name = "self")]
    Self_ {
        /// Action to perform
        #[arg(value_enum)]
        action: SelfAction,
    },

    /// Start a server for orchestrating sandboxes
    #[command(name = "server")]
    Server {
        /// The subcommand to run
        #[command(subcommand)]
        subcommand: ServerSubcommand,
    },

    /// Version of monocore
    #[command(name = "version")]
    Version,
}

/// Subcommands for the server subcommand
#[derive(Debug, Parser)]
pub enum ServerSubcommand {
    /// Start the sandbox server
    Start {
        /// Port to listen on
        #[arg(long)]
        port: Option<u16>,

        /// Path to the namespace directory
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Disable default namespace
        #[arg(long, default_value_t = false)]
        disable_default: bool,
    },

    /// Stop the sandbox server
    Stop,
}

/// Actions for the self subcommand
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SelfAction {
    /// Upgrade monocore
    Upgrade,

    /// Uninstall monocore
    Uninstall,
}

//-------------------------------------------------------------------------------------------------
// Methods
//-------------------------------------------------------------------------------------------------

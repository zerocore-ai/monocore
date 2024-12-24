use super::styles;
use clap::Parser;
use typed_path::Utf8UnixPathBuf;

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
    /// Initialize a new monocore project
    #[command(name = "init")]
    Init {
        /// Path to initialize the project in
        #[arg(value_name = "PATH")]
        path: Option<Utf8UnixPathBuf>,
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

        /// Group volume mappings
        #[arg(long)]
        group_volumes: Vec<String>,

        /// Group environment variables
        #[arg(long)]
        group_envs: Vec<String>,
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
        /// Show sandbox logs
        #[arg(short, long)]
        sandbox: bool,

        /// Name of the component
        #[arg(required = true)]
        name: String,

        /// Follow the logs
        #[arg(long)]
        follow: bool,

        /// Don't use a pager
        #[arg(long)]
        no_pager: bool,

        /// Number of lines to show from the end
        #[arg(long)]
        tail: Option<usize>,

        /// Number of lines to show
        #[arg(long)]
        count: Option<usize>,

        /// Log level
        #[arg(short = 'L')]
        level: Option<String>,
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
        /// Target sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Name of the sandbox
        #[arg(required = true)]
        name: String,

        /// Script to run
        #[arg(long)]
        script: Option<String>,

        /// Additional arguments
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Start a sandbox
    #[command(name = "start")]
    Start {
        /// Target sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Name of the sandbox
        #[arg(required = true)]
        name: String,

        /// Additional arguments
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Open a shell in a sandbox
    #[command(name = "shell")]
    Shell {
        /// Target sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Name of the sandbox
        #[arg(required = true)]
        name: String,
    },

    /// Create a temporary sandbox
    #[command(name = "tmp")]
    Tmp {
        /// Image to use
        #[arg(short, long)]
        image: String,

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

        /// Group volume mappings
        #[arg(long)]
        group_volumes: Vec<String>,

        /// Group environment variables
        #[arg(long)]
        group_envs: Vec<String>,
    },

    /// Install a script from an image
    #[command(name = "install")]
    Install {
        /// Image to install from
        #[arg(short, long)]
        image: String,

        /// Script to install
        #[arg(long)]
        script: Option<String>,

        /// New name for the script
        rename: Option<String>,
    },

    /// Uninstall a script
    #[command(name = "uninstall")]
    Uninstall {
        /// Script to uninstall
        #[arg(long)]
        script: Option<String>,

        /// Image to uninstall from
        #[arg(short, long)]
        image: Option<String>,
    },

    /// Start project sandboxes
    #[command(name = "up")]
    Up {
        /// Target sandboxes
        #[arg(short, long)]
        sandbox: bool,

        /// Target group
        #[arg(short, long)]
        group: bool,

        /// Names of components to start
        names: Vec<String>,
    },

    /// Stop project sandboxes
    #[command(name = "down")]
    Down {
        /// Target sandboxes
        #[arg(short, long)]
        sandbox: bool,

        /// Target group
        #[arg(short, long)]
        group: bool,

        /// Names of components to stop
        names: Vec<String>,
    },

    /// Show running status
    #[command(name = "status")]
    Status {
        /// Target sandboxes
        #[arg(short, long)]
        sandbox: bool,

        /// Target group
        #[arg(short, long)]
        group: bool,

        /// Names of components to check
        names: Vec<String>,
    },

    /// Create or update project's lock file
    #[command(name = "lock")]
    Lock,

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
        /// Image to pull
        #[arg(short, long)]
        image: String,
    },

    /// Push an image
    #[command(name = "push", alias = "publish")]
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

    /// Deploy to cloud
    #[command(name = "deploy")]
    Deploy {
        /// Deploy sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Deploy group
        #[arg(short, long)]
        group: bool,

        /// Name of component to deploy
        name: Option<String>,
    },

    /// Version of monocore
    #[command(name = "version")]
    Version,
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

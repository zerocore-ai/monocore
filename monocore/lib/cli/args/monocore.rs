use std::{error::Error, path::PathBuf};

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
        /// Whether to add a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to add a build sandbox
        #[arg(short, long)]
        build: bool,

        /// Whether to add a group
        #[arg(short, long)]
        group: bool,

        /// Names of components to add
        #[arg(required = true)]
        names: Vec<String>,

        /// Image to use
        #[arg(short, long)]
        image: String,

        /// RAM in MiB
        #[arg(long)]
        ram: Option<u32>,

        /// Number of CPUs
        #[arg(long, alias = "cpu")]
        cpus: Option<u32>,

        /// Volume mappings, format: <host_path>:<container_path>
        #[arg(long = "volume", name = "VOLUME")]
        volumes: Vec<String>,

        /// Port mappings, format: <host_port>:<container_port>
        #[arg(long = "port", name = "PORT")]
        ports: Vec<String>,

        /// Environment variables, format: <key>=<value>
        #[arg(long = "env", name = "ENV")]
        envs: Vec<String>,

        /// Environment file
        #[arg(long)]
        env_file: Option<Utf8UnixPathBuf>,

        /// Dependencies
        #[arg(long)]
        depends_on: Vec<String>,

        /// Working directory
        #[arg(long)]
        workdir: Option<Utf8UnixPathBuf>,

        /// Shell to use
        #[arg(long)]
        shell: Option<String>,

        /// Scripts to add
        #[arg(long = "script", name = "SCRIPT", value_parser = parse_key_val::<String, String>)]
        scripts: Vec<(String, String)>,

        /// Files to import, format: <name>=<path>
        #[arg(long = "import", name = "IMPORT", value_parser = parse_key_val::<String, String>)]
        imports: Vec<(String, String)>,

        /// Files to export, format: <name>=<path>
        #[arg(long = "export", name = "EXPORT", value_parser = parse_key_val::<String, String>)]
        exports: Vec<(String, String)>,

        /// Network reach, options: local, public, any, none
        #[arg(long)]
        reach: Option<String>,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Remove a build, sandbox, or group component from the project
    #[command(name = "remove")]
    Remove {
        /// Whether to remove a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to remove a build sandbox
        #[arg(short, long)]
        build: bool,

        /// Whether to remove a group
        #[arg(short, long)]
        group: bool,

        /// Names of components to remove
        #[arg(required = true)]
        names: Vec<String>,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// List build, sandbox, or group components in the project
    #[command(name = "list")]
    List {
        /// Whether to list sandboxes
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to list build sandboxes
        #[arg(short, long)]
        build: bool,

        /// Whether to list groups
        #[arg(short, long)]
        group: bool,

        /// Project path
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Config path
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Show logs of a running build, sandbox, or group
    #[command(name = "log")]
    Log {
        /// Whether to show logs of a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to show logs of a build sandbox
        #[arg(short, long)]
        build: bool,

        /// Whether to show logs of a group
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
        /// Whether to show a sandbox tree
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to show a build sandbox tree
        #[arg(short, long)]
        build: bool,

        /// Whether to show a group tree
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
        /// Whether to run start or specific script for a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to run start or specific script for a build sandbox
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

        /// Run sandbox in the background
        #[arg(short, long)]
        detach: bool,

        /// Execute a command within the sandbox
        #[arg(short, long)]
        exec: Option<String>,

        /// Additional arguments after `--`
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Start a sandbox
    #[command(name = "start")]
    Start {
        /// Whether to run start script for a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to run start script for a build sandbox
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
        /// Whether to open a shell in a sandbox
        #[arg(short, long)]
        sandbox: bool,

        /// Whether to open a shell in a build sandbox
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
        /// Whether to create a temporary sandbox from an image
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

        /// Volume mappings, format: <host_path>:<container_path>
        #[arg(long = "volume", name = "VOLUME")]
        volumes: Vec<String>,

        /// Port mappings, format: <host_port>:<container_port>
        #[arg(long = "port", name = "PORT")]
        ports: Vec<String>,

        /// Environment variables, format: <key>=<value>
        #[arg(long = "env", name = "ENV")]
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

        /// Whether to start a build sandbox
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

        /// Whether to stop a build sandbox
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

        /// Whether to show a build sandbox
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

        /// Names of components to build
        #[arg(required = true)]
        names: Vec<String>,

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
        /// Whether to push an image
        #[arg(short, long)]
        image: bool,

        /// Whether to push an image group
        #[arg(short = 'G', long)]
        image_group: bool,

        /// Name of the image or image group
        #[arg(required = true)]
        name: String,
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
// Functions: Helpers
//-------------------------------------------------------------------------------------------------

fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;

    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

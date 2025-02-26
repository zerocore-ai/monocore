//! `mcrun` is a polymorphic binary that can operate in two modes: MicroVM or supervisor.
//!
//! # Overview
//!
//! This binary provides a unified interface for running either:
//! - A MicroVM that provides an isolated execution environment
//! - A supervisor process that can manage and monitor child processes
//!
//! ## Usage
//!
//! ### MicroVM Mode
//!
//! To run as a MicroVM:
//! ```bash
//! mcrun microvm \
//!     --log-level=3 \
//!     --root-path=/path/to/rootfs \
//!     --num-vcpus=2 \
//!     --ram-mib=1024 \
//!     --workdir-path=/app \
//!     --exec-path=/usr/bin/python3 \
//!     --mapped-dirs=/host/path:/guest/path \
//!     --port-map=8080:80 \
//!     --env=KEY=VALUE \
//!     -- -m http.server 8080
//! ```
//!
//! ### Supervisor Mode
//!
//! To run as a supervisor:
//! ```bash
//! mcrun supervisor \
//!     --log-dir=/path/to/logs \
//!     --child-name=my_vm \
//!     --sandbox-db-path=/path/to/mcrun.db \
//!     --overlayfs-layer-paths=/path/to/overlayfs/layer1,/path/to/overlayfs/layer2 \
//!     --nfs-host=0.0.0.0 \
//!     --nfs-port=2049 \
//!     --log-level=3 \
//!     --root-path=/path/to/rootfs \
//!     --num-vcpus=2 \
//!     --ram-mib=1024 \
//!     --workdir-path=/app \
//!     --exec-path=/usr/bin/python3 \
//!     --mapped-dirs=/host/path:/guest/path \
//!     --port-map=8080:80 \
//!     --env=KEY=VALUE \
//!     --forward-output \
//!     -- -m http.server 8080
//! ```

use anyhow::Result;
use clap::Parser;
use monocore::{
    cli::{McrunArgs, McrunSubcommand},
    config::{EnvPair, PathPair, PortPair},
    management::supervise,
    vm::MicroVm,
};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = McrunArgs::parse();

    match args.subcommand {
        McrunSubcommand::Microvm {
            log_level,
            root_path,
            num_vcpus,
            ram_mib,
            workdir_path,
            exec_path,
            env,
            mapped_dirs,
            port_map,
            args,
        } => {
            tracing_subscriber::fmt::init();

            // Parse mapped directories
            let mapped_dirs: Vec<PathPair> = mapped_dirs
                .iter()
                .map(|s| s.parse())
                .collect::<Result<_, _>>()?;

            // Parse port mappings
            let port_map: Vec<PortPair> = port_map
                .iter()
                .map(|s| s.parse())
                .collect::<Result<_, _>>()?;

            // Parse environment variables
            let env: Vec<EnvPair> = env.iter().map(|s| s.parse()).collect::<Result<_, _>>()?;

            // Create and configure MicroVM
            let mut builder = MicroVm::builder()
                .root_path(root_path)
                .num_vcpus(num_vcpus)
                .ram_mib(ram_mib)
                .mapped_dirs(mapped_dirs)
                .port_map(port_map)
                .exec_path(exec_path)
                .args(args.iter().map(|s| s.as_str()))
                .env(env);

            // Set log level if provided
            if let Some(log_level) = log_level {
                builder = builder.log_level(log_level.try_into()?);
            }

            // Set working directory if provided
            if let Some(workdir_path) = workdir_path {
                builder = builder.workdir_path(workdir_path);
            }

            // Build and start the MicroVM
            let vm = builder.build()?;

            tracing::info!("starting µvm");
            vm.start()?;
        }
        McrunSubcommand::Supervisor {
            log_dir,
            child_name,
            sandbox_db_path,
            overlayfs_layer_paths,
            nfs_host,
            nfs_port,
            root_path,
            log_level,
            forward_output,
            num_vcpus,
            ram_mib,
            workdir_path,
            exec_path,
            env,
            mapped_dirs,
            port_map,
            args,
        } => {
            tracing_subscriber::fmt::init();
            supervise::start_supervision(
                log_dir,
                child_name,
                sandbox_db_path,
                forward_output,
                overlayfs_layer_paths,
                nfs_host,
                nfs_port,
                root_path,
                num_vcpus,
                ram_mib,
                workdir_path,
                exec_path,
                env,
                mapped_dirs,
                port_map,
                log_level,
                args,
            )
            .await?;
        }
    }

    // NOTE: Force exit to make process actually exit when supervisor runs a child in TTY mode.
    // Otherwise, the process will not exit by itself and will wait for enter key to be pressed.
    std::process::exit(0);
}

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
//!     --native-rootfs=/path/to/rootfs \
//!     --overlayfs-rootfs=/path/to/rootfs \
//!     --num-vcpus=2 \
//!     --ram-mib=1024 \
//!     --workdir-path=/app \
//!     --exec-path=/usr/bin/python3 \
//!     --mapped-dirs=/host/path:/guest/path \
//!     --port-maps=8080:80 \
//!     --envs=KEY=VALUE \
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
//!     --log-level=3 \
//!     --native-rootfs=/path/to/rootfs \
//!     --overlayfs-rootfs=/path/to/rootfs \
//!     --num-vcpus=2 \
//!     --ram-mib=1024 \
//!     --workdir-path=/app \
//!     --exec-path=/usr/bin/python3 \
//!     --mapped-dirs=/host/path:/guest/path \
//!     --port-maps=8080:80 \
//!     --envs=KEY=VALUE \
//!     --forward-output \
//!     -- -m http.server 8080
//! ```

use std::env;

use anyhow::Result;
use chrono::Duration;
use clap::Parser;
use monocore::{
    cli::{McrunArgs, McrunSubcommand},
    config::{EnvPair, PathPair, PortPair},
    runtime::MicroVmMonitor,
    vm::{MicroVm, Rootfs},
};
use monoutils::runtime::Supervisor;

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
            native_rootfs,
            overlayfs_layer,
            num_vcpus,
            ram_mib,
            workdir_path,
            exec_path,
            env,
            mapped_dir,
            port_map,
            args,
        } => {
            tracing_subscriber::fmt::init();

            // Check that only one of native_rootfs or overlayfs_rootfs is provided
            let rootfs = match (native_rootfs, overlayfs_layer.is_empty()) {
                (Some(path), true) => Rootfs::Native(path),
                (None, false) => Rootfs::Overlayfs(overlayfs_layer),
                (Some(_), false) => {
                    anyhow::bail!("Cannot specify both native_rootfs and overlayfs_rootfs")
                }
                (None, true) => {
                    anyhow::bail!("Must specify either native_rootfs or overlayfs_rootfs")
                }
            };

            tracing::info!("rootfs: {:#?}", rootfs);

            // Parse mapped directories
            let mapped_dir: Vec<PathPair> = mapped_dir
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
            let mut builder = MicroVm::builder().rootfs(rootfs).exec_path(exec_path);

            // Set num vcpus if provided
            if let Some(num_vcpus) = num_vcpus {
                builder = builder.num_vcpus(num_vcpus);
            }

            // Set ram mib if provided
            if let Some(ram_mib) = ram_mib {
                builder = builder.ram_mib(ram_mib);
            }

            // Set log level if provided
            if let Some(log_level) = log_level {
                builder = builder.log_level(log_level.try_into()?);
            }

            // Set working directory if provided
            if let Some(workdir_path) = workdir_path {
                builder = builder.workdir_path(workdir_path);
            }

            // Set mapped dirs if provided
            if !mapped_dir.is_empty() {
                builder = builder.mapped_dirs(mapped_dir);
            }

            // Set port map if provided
            if !port_map.is_empty() {
                builder = builder.port_map(port_map);
            }

            // Set env if provided
            if !env.is_empty() {
                builder = builder.env(env);
            }

            // Set args if provided
            if !args.is_empty() {
                builder = builder.args(args.iter().map(|s| s.as_str()));
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
            log_level,
            forward_output,
            native_rootfs,
            overlayfs_layer,
            num_vcpus,
            ram_mib,
            workdir_path,
            exec_path,
            env,
            mapped_dir,
            port_map,
            args,
        } => {
            tracing_subscriber::fmt::init();
            tracing::info!("setting up supervisor");

            // Get current executable path
            let child_exe = env::current_exe()?;

            // Get supervisor PID
            let supervisor_pid = std::process::id();

            // Get rootfs
            let rootfs = match (&native_rootfs, &overlayfs_layer.is_empty()) {
                (Some(path), true) => Rootfs::Native(path.clone()),
                (None, false) => Rootfs::Overlayfs(overlayfs_layer.clone()),
                (Some(_), false) => {
                    anyhow::bail!("Cannot specify both native_rootfs and overlayfs_rootfs")
                }
                (None, true) => {
                    anyhow::bail!("Must specify either native_rootfs or overlayfs_rootfs")
                }
            };

            // Create microvm monitor
            let process_monitor = MicroVmMonitor::new(
                supervisor_pid,
                sandbox_db_path,
                log_dir.clone(),
                rootfs.clone(),
                Duration::days(1),
                forward_output,
            )
            .await?;

            // Compose child arguments
            let mut child_args = vec!["microvm".to_string(), format!("--exec-path={}", exec_path)];

            // Set num vcpus if provided
            if let Some(num_vcpus) = num_vcpus {
                child_args.push(format!("--num-vcpus={}", num_vcpus));
            }

            // Set ram mib if provided
            if let Some(ram_mib) = ram_mib {
                child_args.push(format!("--ram-mib={}", ram_mib));
            }

            // Set workdir path if provided
            if let Some(workdir_path) = workdir_path {
                child_args.push(format!("--workdir-path={}", workdir_path));
            }

            // Set native rootfs if provided
            if let Some(native_rootfs) = native_rootfs {
                child_args.push(format!("--native-rootfs={}", native_rootfs.display()));
            }

            // Set overlayfs rootfs if provided
            if !overlayfs_layer.is_empty() {
                for path in overlayfs_layer {
                    child_args.push(format!("--overlayfs-layer={}", path.display()));
                }
            }

            // Set env if provided
            if !env.is_empty() {
                for env in env {
                    child_args.push(format!("--env={}", env));
                }
            }

            // Set mapped dirs if provided
            if !mapped_dir.is_empty() {
                for dir in mapped_dir {
                    child_args.push(format!("--mapped-dir={}", dir));
                }
            }

            // Set port map if provided
            if !port_map.is_empty() {
                for port_map in port_map {
                    child_args.push(format!("--port-map={}", port_map));
                }
            }

            // Set log level if provided
            if let Some(log_level) = log_level {
                child_args.push(format!("--log-level={}", log_level));
            }

            // Set args if provided
            if !args.is_empty() {
                child_args.push("--".to_string());
                for arg in args {
                    child_args.push(arg);
                }
            }

            // Compose child environment variables
            let mut child_envs = Vec::<(String, String)>::new();

            // Only pass RUST_LOG if it's set in the environment
            if let Ok(rust_log) = std::env::var("RUST_LOG") {
                tracing::debug!("using existing RUST_LOG: {:?}", rust_log);
                child_envs.push(("RUST_LOG".to_string(), rust_log));
            }

            // Create and start supervisor
            let mut supervisor = Supervisor::new(
                child_exe,
                child_args,
                child_envs,
                child_name,
                log_dir,
                process_monitor,
            );

            supervisor.start().await?;
        }
    }

    // NOTE: Force exit to make process actually exit when supervisor runs a child in TTY mode.
    // Otherwise, the process will not exit by itself and will wait for enter key to be pressed.
    std::process::exit(0);
}

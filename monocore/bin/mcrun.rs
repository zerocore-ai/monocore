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

use std::env;

use anyhow::Result;
use chrono::Duration;
use clap::Parser;
use monocore::{
    cli::{McrunArgs, McrunSubcommand},
    config::{EnvPair, PathPair, PortPair},
    runtime::MicroVmMonitor,
    vm::MicroVm,
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
            log_level,
            forward_output,
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
            tracing::info!("setting up supervisor");

            // Get current executable path
            let child_exe = env::current_exe()?;

            // Get supervisor PID
            let supervisor_pid = std::process::id();

            // Create microvm monitor
            let process_monitor = MicroVmMonitor::new(
                supervisor_pid,
                sandbox_db_path,
                log_dir.clone(),
                root_path.clone(),
                Duration::days(1),
                forward_output,
            )
            .await?;

            // Compose child arguments
            let mut child_args = vec![
                "microvm".to_string(),
                format!("--root-path={}", root_path.display()),
                format!("--num-vcpus={}", num_vcpus),
                format!("--ram-mib={}", ram_mib),
                format!("--workdir-path={}", workdir_path.unwrap_or_default()),
                format!("--exec-path={}", exec_path),
            ];

            // Set env if provided
            if !env.is_empty() {
                child_args.push(format!("--env={}", env.join(",")));
            }

            // Set mapped dirs if provided
            if !mapped_dirs.is_empty() {
                child_args.push(format!("--mapped-dirs={}", mapped_dirs.join(",")));
            }

            // Set port map if provided
            if !port_map.is_empty() {
                child_args.push(format!("--port-map={}", port_map.join(",")));
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

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
//!     --root-path=/path/to/rootfs \
//!     --ram-mib=1024 \
//!     --mapped-dirs=/host/path:/guest/path \
//!     --port-map=8080:80 \
//!     --workdir-path=/app \
//!     --exec-path=/usr/bin/python3 \
//!     --args="-m" --args="http.server" --args="8080"
//! ```
//!
//! ### Supervisor Mode
//!
//! To run as a supervisor:
//! ```bash
//! mcrun supervisor \
//!     --log-dir=/path/to/logs \
//!     --child-name=my_vm \
//!     --db-path=/path/to/mcrun.db
//! ```

use std::env;

use anyhow::Result;
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
    // Initialize logging without ANSI colors
    tracing_subscriber::fmt().with_ansi(false).init();

    // Parse command line arguments
    let args = McrunArgs::parse();

    match args.subcommand {
        McrunSubcommand::Microvm {
            root_path,
            num_vcpus,
            ram_mib,
            mapped_dirs,
            port_map,
            workdir_path,
            exec_path,
            args,
            env,
        } => {
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

            if let Some(workdir_path) = workdir_path {
                builder = builder.workdir_path(workdir_path);
            }

            // Build and start the MicroVM
            let vm = builder.build()?;

            tracing::info!("Starting MicroVM");
            vm.start()?;
        }
        McrunSubcommand::Supervisor {
            log_dir,
            child_name,
            sandbox_db_path,
        } => {
            // Get current executable path
            let child_exe = env::current_exe()?;

            // Get supervisor PID
            let supervisor_pid = std::process::id();

            // Create microvm monitor
            let process_monitor =
                MicroVmMonitor::new(supervisor_pid, sandbox_db_path, log_dir.clone()).await?;

            // Compose child arguments - these are placeholders that will be overridden
            let child_args = vec![
                "microvm".to_string(),
                "--root-path=/".to_string(),
                "--ram-mib=512".to_string(),
            ];

            // Compose child environment variables
            let child_envs = vec![("RUST_LOG", "info")];

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

    Ok(())
}

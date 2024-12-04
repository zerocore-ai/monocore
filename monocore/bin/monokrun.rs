use std::{env, net::Ipv4Addr, path::PathBuf};

use monocore::{
    config::{Group, Service},
    runtime::Supervisor,
    vm::MicroVm,
    MonocoreError, MonocoreResult,
};
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info};

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

/// Entry point for the runtime supervisor and microvm subprocess.
///
/// Handles both supervisor and microvm subprocess modes based on command line arguments.
///
/// # Arguments
///
/// Expected arguments for supervisor mode:
/// ```text
/// monokrun --run-supervisor <service_json> <group_json> <group_ip> <rootfs_path>
/// ```
///
/// Expected arguments for subprocess mode:
/// ```text
/// monokrun --run-microvm <service_json> <env_json> <local_only> <group_ip> <rootfs_path>
/// ```
#[tokio::main]
pub async fn main() -> MonocoreResult<()> {
    let args: Vec<_> = env::args().collect();

    // Check for microvm mode first
    if args.len() == 7 && args[1] == "--run-microvm" {
        // Handle microvm mode
        let service: Service = serde_json::from_str(&args[2])?;
        let group: Group = serde_json::from_str(&args[3])?;
        let local_only: bool = serde_json::from_str(&args[4])?;
        let group_ip: Option<Ipv4Addr> = serde_json::from_str(&args[5])?;
        let rootfs_path = PathBuf::from(&args[6]);

        // Resolve environment variables
        let env_pairs = service.resolve_environment_variables(&group)?;
        let volumes = service.resolve_volumes(&group)?;

        // Set up micro VM options
        let mut builder = MicroVm::builder()
            .root_path(rootfs_path)
            .num_vcpus(service.get_cpus())
            .ram_mib(service.get_ram())
            .port_map(service.get_port().cloned().into_iter())
            .workdir_path(service.get_workdir().unwrap_or("/"))
            .exec_path(service.get_command().unwrap_or("/bin/sh"))
            .args(service.get_args().iter().map(|s| s.as_str()))
            .env(env_pairs)
            .mapped_dirs(volumes)
            .local_only(local_only);

        // Only set assigned_ip if Some
        if let Some(ip) = group_ip {
            builder = builder.assigned_ip(ip);
        }

        let microvm = builder.build()?;

        microvm.start()?;
        return Ok(());
    }

    // Check for supervisor mode
    if args.len() == 6 && args[1] == "--run-supervisor" {
        tracing_subscriber::fmt().init();

        let service: Service = serde_json::from_str(&args[2])?;
        let group: Group = serde_json::from_str(&args[3])?;
        let group_ip: Option<Ipv4Addr> = serde_json::from_str(&args[4])?;
        let rootfs_path = PathBuf::from(&args[5]);

        // Create and start the supervisor
        let mut supervisor = Supervisor::new(service, group, group_ip, rootfs_path).await?;

        // Set up signal handler for graceful shutdown
        let mut term_signal = signal(SignalKind::terminate())?;

        // Start the supervisor and get the join handle
        let supervisor_handle = supervisor.start().await?;

        tokio::select! {
            result = supervisor_handle => {
                match result {
                    Ok(result) => {
                        info!("Supervisor exited normally");
                        result?;
                    }
                    Err(e) => {
                        error!("Supervisor task failed: {}", e);
                        return Err(MonocoreError::SupervisorError(e.to_string()));
                    }
                }
            }
            _ = term_signal.recv() => {
                info!("Received SIGTERM signal, initiating graceful shutdown");
                supervisor.stop().await?;
            }
        }

        return Ok(());
    }

    // If we get here, no valid subcommand was provided
    Err(MonocoreError::InvalidSupervisorArgs(
        "Usage: monokrun --run-supervisor <service_json> <group_json> <group_ip> <rootfs_path>\n       monokrun --run-microvm <service_json> <env_json> <local_only> <group_ip> <rootfs_path>".into(),
    ))
}

use std::env;

use clap::{CommandFactory, Parser};
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    config::Monocore,
    orchestration::Orchestrator,
    utils::{self, OCI_SUBDIR, ROOTFS_SUBDIR},
    MonocoreError, MonocoreResult,
};
use tokio::fs;
use tracing::info;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The name of the supervisor executable
const SUPERVISOR_EXE: &str = "monokrun";

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    // Parse command line arguments
    let args = MonocoreArgs::parse();

    // Initialize logging with appropriate verbosity
    args.init_logging();

    match args.subcommand {
        Some(MonocoreSubcommand::Up {
            file,
            group,
            home_dir,
        }) => {
            if !file.exists() {
                return Err(MonocoreError::ConfigNotFound(file.display().to_string()));
            }

            // Read and parse config
            let config_str = fs::read_to_string(&file).await?;
            let mut config: Monocore = toml::from_str(&config_str)?;

            // Filter services by group if specified
            if let Some(group_name) = group {
                let services = config
                    .get_services()
                    .iter()
                    .filter(|s| s.get_group().map_or(false, |g| g == group_name))
                    .cloned()
                    .collect::<Vec<_>>();
                config = Monocore::builder()
                    .services(services)
                    .groups(config.get_groups().clone())
                    .build()?;
            }

            // Get current executable path for supervisor
            let current_exe = env::current_exe()?;
            let supervisor_path = current_exe.parent().unwrap().join(SUPERVISOR_EXE);

            // Try to load existing orchestrator state first
            let mut orchestrator = match Orchestrator::load(&home_dir, &supervisor_path).await {
                Ok(orchestrator) => {
                    info!("Loaded existing orchestrator state");
                    orchestrator
                }
                Err(e) => {
                    info!("Creating new orchestrator: {}", e);
                    Orchestrator::new(&home_dir, &supervisor_path).await?
                }
            };

            // Start services
            orchestrator.up(config).await?;
        }

        Some(MonocoreSubcommand::Down {
            file: _,
            group,
            home_dir,
        }) => {
            let current_exe = env::current_exe()?;
            let supervisor_path = current_exe.parent().unwrap().join(SUPERVISOR_EXE);
            let mut orchestrator = Orchestrator::load(&home_dir, &supervisor_path).await?;

            if let Some(group_name) = group {
                // Get all services in the group
                let services = orchestrator
                    .get_running_services()
                    .keys()
                    .filter(|&name| {
                        orchestrator
                            .get_service(name)
                            .map_or(false, |s| s.get_group() == Some(&group_name))
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                for service in services {
                    orchestrator.down(Some(&service)).await?;
                }
            } else {
                orchestrator.down(None).await?;
            }
        }

        Some(MonocoreSubcommand::Status {}) => {
            let current_exe = env::current_exe()?;
            let supervisor_path = current_exe.parent().unwrap().join(SUPERVISOR_EXE);
            let rootfs_dir = monocore::utils::monocore_home_path().join(ROOTFS_SUBDIR);
            let orchestrator = Orchestrator::load(&rootfs_dir, &supervisor_path).await?;
            let statuses = orchestrator.status().await?;

            println!();
            println!(
                "{:<15} {:<10} {:<8} {:<8} {:<10} {:<10} {:<15} {:<15} {:<10} {:<10}",
                "Service",
                "Group",
                "vCPUs",
                "RAM",
                "Sup PID",
                "VM PID",
                "Status",
                "Assigned IP",
                "CPU Usage",
                "Mem Usage"
            );
            println!("{:-<120}", "");

            for status in statuses {
                // Get supervisor PID from orchestrator's running_services map
                let sup_pid = orchestrator
                    .get_running_services()
                    .get(status.get_name())
                    .copied()
                    .unwrap_or(0);

                // Format CPU as percentage - dereference the f64
                let cpu_pct = (*status.get_state().get_metrics().get_cpu_usage() * 100.0).ceil();
                // Format memory in MiB - dereference the u64 before casting
                let mem_mib = (*status.get_state().get_metrics().get_memory_usage() as f64)
                    / (1024.0 * 1024.0);

                println!(
                    "{:<15} {:<10} {:<8} {:<8} {:<10} {:<10} {:<15} {:<15} {:<10} {:<10}",
                    status.get_name(),
                    status.get_state().get_group().get_name(),
                    status.get_state().get_service().get_cpus(),
                    status.get_state().get_service().get_ram(),
                    sup_pid,
                    status.get_pid().unwrap_or(0),
                    format!("{:?}", status.get_state().get_status()),
                    status
                        .get_state()
                        .get_group_ip()
                        .map_or_else(|| std::net::Ipv4Addr::LOCALHOST, |ip| ip),
                    format!("{}%", cpu_pct as u64),
                    format!("{}MiB", mem_mib.ceil() as u64)
                );
            }
            println!();
        }

        Some(MonocoreSubcommand::Pull { image, home_dir }) => {
            let oci_dir = home_dir.join(OCI_SUBDIR);
            fs::create_dir_all(&oci_dir).await?;
            utils::pull_docker_image(&oci_dir, &image).await?;
            info!("Successfully pulled {}", image);
        }

        Some(MonocoreSubcommand::Remove {
            services,
            group,
            home_dir,
        }) => {
            let current_exe = env::current_exe()?;
            let supervisor_path = current_exe.parent().unwrap().join(SUPERVISOR_EXE);
            let mut orchestrator = Orchestrator::load(&home_dir, &supervisor_path).await?;

            match (services.is_empty(), group) {
                (false, None) => {
                    // Remove specific services
                    orchestrator.remove_services(&services).await?;
                    info!("Successfully removed services: {}", services.join(", "));
                }
                (true, Some(group_name)) => {
                    // Remove all services in group
                    orchestrator.remove_group(&group_name).await?;
                    info!("Successfully removed services from group: {}", group_name);
                }
                (false, Some(_)) => {
                    return Err(MonocoreError::InvalidArgument(
                        "Cannot specify both services and group".to_string(),
                    ));
                }
                (true, None) => {
                    return Err(MonocoreError::InvalidArgument(
                        "Must specify either services or group".to_string(),
                    ));
                }
            }
        }

        None => {
            MonocoreArgs::command().print_help()?;
            std::process::exit(0);
        }
    }

    Ok(())
}

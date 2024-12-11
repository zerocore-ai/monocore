use std::{env, io::Write};

use clap::{CommandFactory, Parser};
use futures::StreamExt;
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    config::Monocore,
    orchestration::Orchestrator,
    server::{self, ServerState},
    utils::{self, OCI_SUBDIR, ROOTFS_SUBDIR},
    MonocoreError, MonocoreResult,
};
use serde::de::DeserializeOwned;
use tokio::{fs, io::AsyncWriteExt, process::Command, signal::unix::SignalKind};
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
            tracing::info!("Home dir: {}", home_dir.display());
            if !file.exists() {
                return Err(MonocoreError::ConfigNotFound(file.display().to_string()));
            }

            // Parse the config file
            let mut config: Monocore = parse_config_file(
                &file,
                file.extension().unwrap_or_default().to_str().unwrap(),
            )
            .await?;

            // Filter services by group if specified
            if let Some(group_name) = group {
                let services = config
                    .get_services()
                    .iter()
                    .filter(|s| s.get_group().is_some_and(|g| g == group_name))
                    .cloned()
                    .collect::<Vec<_>>();
                config = Monocore::builder()
                    .services(services)
                    .groups(config.get_groups().to_vec())
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
                            .is_some_and(|s| s.get_group() == Some(&group_name))
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
                let cpu_pct = status.get_state().get_metrics().get_cpu_usage();
                // Format memory in MiB - dereference the u64 before casting
                let mem_mib = (status.get_state().get_metrics().get_memory_usage() as f64)
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
                    format!("{:.2}%", cpu_pct),
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

        Some(MonocoreSubcommand::Serve { port, home_dir }) => {
            let current_exe = env::current_exe()?;
            let supervisor_path = current_exe.parent().unwrap().join(SUPERVISOR_EXE);

            let state = ServerState::new(home_dir, supervisor_path).await?;
            let app = server::create_router(state);

            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            info!("Starting server on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app)
                .await
                .map_err(MonocoreError::custom)?;
        }

        Some(MonocoreSubcommand::Log {
            service,
            lines,
            no_pager,
            follow,
            home_dir,
        }) => {
            let current_exe = env::current_exe()?;
            let supervisor_path = current_exe.parent().unwrap().join(SUPERVISOR_EXE);
            let orchestrator = Orchestrator::load(&home_dir, &supervisor_path).await?;

            let mut log_stream = orchestrator.view_logs(&service, lines, follow).await?;

            if follow || no_pager {
                // Set up Ctrl+C handler
                let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt())?;

                // Print directly to stdout for follow mode or when no pager is requested
                loop {
                    tokio::select! {
                        maybe_line = log_stream.next() => {
                            match maybe_line {
                                Some(line) => {
                                    print!("{}", line?);
                                    std::io::stdout().flush()?;
                                }
                                None => break,
                            }
                        }
                        _ = sigint.recv() => {
                            break;
                        }
                    }
                }
            } else {
                // Collect all content for pager mode
                let mut content = String::new();
                while let Some(line) = log_stream.next().await {
                    content.push_str(&line?);
                }

                let mut less = Command::new("less")
                    .arg("-R") // Handle ANSI color codes
                    .stdin(std::process::Stdio::piped())
                    .spawn()?;

                if let Some(mut stdin) = less.stdin.take() {
                    stdin.write_all(content.as_bytes()).await?;
                }

                less.wait().await?;
            }
        }

        None => {
            MonocoreArgs::command().print_help()?;
            std::process::exit(0);
        }
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Function: *
//--------------------------------------------------------------------------------------------------

async fn parse_config_file<T: DeserializeOwned>(
    file_path: &std::path::Path,
    r#type: &str,
) -> MonocoreResult<T> {
    let content = fs::read_to_string(file_path).await?;

    match r#type {
        "json" => serde_json::from_str(&content).map_err(MonocoreError::SerdeJson),
        "yaml" | "yml" => serde_yaml::from_str(&content).map_err(MonocoreError::SerdeYaml),
        _ => toml::from_str(&content).map_err(MonocoreError::Toml),
    }
}

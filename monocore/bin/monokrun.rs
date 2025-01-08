use monocore::MonocoreResult;

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
/// monokrun --run-microvm <service_json> <group_json> <group_ip> <rootfs_path> <local_only>
/// ```
#[tokio::main]
pub async fn main() -> MonocoreResult<()> {
    // let args: Vec<_> = env::args().collect();

    // // Check for microvm mode first
    // if args.len() == 7 && args[1] == "--run-microvm" {
    //     // Handle microvm mode
    //     let service: Service = serde_json::from_str(&args[2])?;
    //     let group: Group = serde_json::from_str(&args[3])?;
    //     let group_ip: Option<Ipv4Addr> = serde_json::from_str(&args[4])?;
    //     let rootfs_path = PathBuf::from(&args[5]);
    //     let local_only: bool = serde_json::from_str(&args[6])?;

    //     // Resolve environment variables
    //     let env_pairs = service.resolve_environment_variables(&group)?;
    //     let volumes = service.resolve_volumes(&group)?;

    //     // Set up micro VM options
    //     let mut builder = MicroVm::builder()
    //         .root_path(rootfs_path)
    //         .num_vcpus(service.get_cpus())
    //         .ram_mib(service.get_ram())
    //         .port_map(service.get_ports().iter().cloned())
    //         .workdir_path(service.get_workdir().unwrap_or("/"))
    //         .exec_path(service.get_command().unwrap_or("/bin/sh"))
    //         .args(service.get_args().iter().map(|s| s.as_str()))
    //         .env(env_pairs)
    //         .mapped_dirs(volumes)
    //         .local_only(local_only);

    //     // Only set assigned_ip if Some
    //     if let Some(ip) = group_ip {
    //         builder = builder.assigned_ip(ip);
    //     }

    //     let microvm = builder.build()?;

    //     microvm.start()?;
    //     return Ok(());
    // }

    // // Check for supervisor mode
    // if args.len() == 7 && args[1] == "--run-supervisor" {
    //     let service: Service = serde_json::from_str(&args[2])?;
    //     let group: Group = serde_json::from_str(&args[3])?;
    //     let group_ip: Option<Ipv4Addr> = serde_json::from_str(&args[4])?;
    //     let rootfs_path = PathBuf::from(&args[5]);
    //     let home_dir = &args[6];

    //     // Create a rotating log file that automatically rotates when reaching max size
    //     let rotating_log = RotatingLog::new(
    //         PathBuf::from(home_dir)
    //             .join(LOG_SUBDIR)
    //             .join(SUPERVISORS_LOG_FILENAME),
    //         None,
    //     )
    //     .await?;

    //     // Bridge between our async rotating log and tracing's sync writer requirement
    //     let sync_writer = rotating_log.get_sync_writer();

    //     // Create a non-blocking writer to prevent logging from blocking execution
    //     let (non_blocking, _guard) = tracing_appender::non_blocking(sync_writer);

    //     // Configure log level filtering from environment variables
    //     let env_filter = EnvFilter::try_from_default_env()
    //         .or_else(|_| EnvFilter::try_new("debug"))
    //         .unwrap();

    //     // Configure file output format without ANSI colors or target field
    //     let file_layer = tracing_subscriber::fmt::layer()
    //         .with_writer(non_blocking)
    //         .with_ansi(false)
    //         .with_target(false)
    //         .with_file(true);

    //     // Set up the global tracing subscriber
    //     tracing_subscriber::registry()
    //         .with(env_filter)
    //         .with(file_layer)
    //         .init();

    //     // Create and start the supervisor
    //     let mut supervisor =
    //         Supervisor::new(home_dir, service, group, group_ip, rootfs_path).await?;

    //     // Set up signal handler for graceful shutdown
    //     let mut term_signal = signal(SignalKind::terminate())?;

    //     // Start the supervisor and get the join handle
    //     let supervisor_handle = supervisor.start().await?;

    //     tokio::select! {
    //         result = supervisor_handle => {
    //             match result {
    //                 Ok(result) => {
    //                     info!("Supervisor exited normally");
    //                     result?;
    //                 }
    //                 Err(e) => {
    //                     error!("Supervisor task failed: {}", e);
    //                     return Err(MonocoreError::SupervisorError(e.to_string()));
    //                 }
    //             }
    //         }
    //         _ = term_signal.recv() => {
    //             info!("Received SIGTERM signal, initiating graceful shutdown");
    //             supervisor.stop().await?;
    //         }
    //     }

    //     return Ok(());
    // }

    // // If we get here, no valid subcommand was provided
    // Err(MonocoreError::InvalidSupervisorArgs(
    //     "Usage: monokrun --run-supervisor <service_json> <group_json> <group_ip> <rootfs_path> <home_dir>\n       monokrun --run-microvm <service_json> <group_json> <group_ip> <rootfs_path> <home_dir> <local_only>>".into(),
    // ))
    Ok(())
}

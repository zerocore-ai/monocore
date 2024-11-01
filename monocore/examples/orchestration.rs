//! If you are trying to run this example, please make sure to run `make example runtime_lifecycle` from
//! the `monocore` subdirectory

use monocore::{
    config::{Group, Monocore, Service},
    orchestration::{LogRetentionPolicy, Orchestrator},
};
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Use the architecture-specific build directory
    let rootfs_path = format!("build/rootfs-alpine-{}", get_current_arch());

    // Path to supervisor binary - adjust this path as needed
    let supervisor_path = "../target/release/monokrun";

    // Create a test configuration
    let config = create_test_config()?;

    // Alternatively, use the convenience method
    let log_retention_policy = LogRetentionPolicy::with_max_age_weeks(1);

    let mut orchestrator = Orchestrator::with_log_retention_policy(
        config,
        rootfs_path,
        supervisor_path,
        log_retention_policy,
    )
    .await?;

    // You can also manually trigger log cleanup
    orchestrator.cleanup_old_logs().await?;

    // Start all services
    println!("Starting all services...");
    orchestrator.up(None).await?;

    // Wait for a moment to let services start
    time::sleep(Duration::from_secs(60)).await;

    // Get status of all services
    println!("\nChecking service status...");
    let statuses = orchestrator.status().await?;

    // Print table headers
    println!(
        "{:<15} {:<10} {:<15} {:<12} {:<14}",
        "Service", "PID", "Status", "CPU Usage", "Memory Usage"
    );
    println!(
        "{:-<15} {:-<10} {:-<15} {:-<12} {:-<14}",
        "", "", "", "", ""
    );

    // Print each service status as a table row
    for status in statuses {
        println!(
            "{:<15} {:<10} {:<15} {:<12} {:<14}",
            status.get_name(),
            status.get_pid().unwrap_or(0),
            format!("{:?}", status.get_state().get_status()),
            status.get_state().get_metrics().get_cpu_usage(),
            status.get_state().get_metrics().get_memory_usage()
        );
    }

    // Stop all services
    println!("\nStopping all services...");
    orchestrator.down(None).await?;

    Ok(())
}

// Add this function to determine the current architecture
fn get_current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        panic!("Unsupported architecture")
    }
}

// Create a simple test configuration that runs long-running Alpine Linux commands
fn create_test_config() -> anyhow::Result<Monocore> {
    // Create the main group
    let main_group = Group::builder().name("main".to_string()).build();

    // Create a service that runs 'tail -f /dev/null' command (keeps running indefinitely)
    let tail_service = Service::builder_default()
        .name("tail-service")
        .base("alpine:latest")
        .group("main")
        .command("/usr/bin/tail")
        .argv(["-f".to_string(), "/dev/null".to_string()])
        .build();

    // Create a service that runs 'sleep infinity' command (keeps running indefinitely)
    let sleep_service = Service::builder_default()
        .name("sleep-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sleep")
        .argv(vec!["infinity".to_string()])
        .build();

    // Create the Monocore configuration
    let config = Monocore::builder()
        .services(vec![tail_service, sleep_service])
        .groups(vec![main_group])
        .build()?;

    Ok(config)
}

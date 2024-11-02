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

    // Create orchestrator with log retention policy
    let log_retention_policy = LogRetentionPolicy::with_max_age_weeks(1);
    let mut orchestrator =
        Orchestrator::with_log_retention_policy(rootfs_path, supervisor_path, log_retention_policy)
            .await?;

    // Create initial configuration
    let initial_config = create_initial_config()?;

    // Start initial services
    println!("Starting initial services...");
    orchestrator.up(initial_config).await?;

    // Wait a bit to let services start
    time::sleep(Duration::from_secs(10)).await;
    print_service_status(&orchestrator).await?;

    // Create updated configuration with modified service and new service
    let updated_config = create_updated_config()?;

    // Update services
    println!("\nUpdating services...");
    orchestrator.up(updated_config).await?;

    // Wait a bit to let services update
    time::sleep(Duration::from_secs(10)).await;
    print_service_status(&orchestrator).await?;

    // Stop all services
    println!("\nStopping all services...");
    orchestrator.down(None).await?;

    Ok(())
}

// Helper function to print service status
async fn print_service_status(orchestrator: &Orchestrator) -> anyhow::Result<()> {
    println!("\nCurrent service status:");
    let statuses = orchestrator.status().await?;

    println!(
        "{:<15} {:<10} {:<15} {:<12} {:<14}",
        "Service", "PID", "Status", "CPU Usage", "Memory Usage"
    );
    println!("{:-<67}", "");

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
    Ok(())
}

// Create initial configuration with two services
fn create_initial_config() -> anyhow::Result<Monocore> {
    // Create the main group
    let main_group = Group::builder().name("main".to_string()).build();

    // Create initial services
    let tail_service = Service::builder_default()
        .name("tail-service")
        .base("alpine:latest")
        .group("main")
        .command("/usr/bin/tail")
        .args(["-f".to_string(), "/dev/null".to_string()])
        .build();

    let sleep_service = Service::builder_default()
        .name("sleep-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sleep")
        .args(vec!["infinity".to_string()])
        .build();

    // Create the Monocore configuration
    let config = Monocore::builder()
        .services(vec![tail_service, sleep_service])
        .groups(vec![main_group])
        .build()?;

    Ok(config)
}

// Create updated configuration with modified service and new service
fn create_updated_config() -> anyhow::Result<Monocore> {
    // Create the main group
    let main_group = Group::builder().name("main".to_string()).build();

    // Create modified tail service (changed args)
    let tail_service = Service::builder_default()
        .name("tail-service")
        .base("alpine:latest")
        .group("main")
        .command("/usr/bin/tail")
        .args(["-f".to_string(), "/etc/hosts".to_string()]) // Changed from /dev/null to /etc/hosts
        .build();

    // Keep sleep service unchanged
    let sleep_service = Service::builder_default()
        .name("sleep-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sleep")
        .args(vec!["infinity".to_string()])
        .build();

    // Add new echo service
    let echo_service = Service::builder_default()
        .name("echo-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sh")
        .args(vec![
            "-c".to_string(),
            "while true; do echo 'Hello from echo service'; sleep 5; done".to_string(),
        ])
        .build();

    // Create the updated Monocore configuration
    let config = Monocore::builder()
        .services(vec![tail_service, sleep_service, echo_service])
        .groups(vec![main_group])
        .build()?;

    Ok(config)
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

//! If you are trying to run this example, please make sure to run `make example orchestration_basic` from
//! the `monocore` subdirectory.
//!
//! This example demonstrates basic orchestration capabilities:
//! - Creating and managing multiple services
//! - Service updates and configuration changes
//! - Monitoring service status and metrics
//!
//! To run the example:
//! ```bash
//! make example orchestration_basic
//! ```
//!
//! The example will:
//! 1. Start initial services (tail-service and sleep-service)
//! 2. Wait 10 seconds and show their status
//! 3. Update the configuration:
//!    - Modify tail-service to watch /etc/hosts instead of /dev/null
//!    - Add a new echo-service
//! 4. Wait 10 seconds and show updated status
//! 5. Stop all services

use monocore::{
    config::{Group, Monocore, Service},
    orchestration::{LogRetentionPolicy, Orchestrator},
};
use std::{net::Ipv4Addr, time::Duration};
use tokio::time;

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with debug level by default
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Use the architecture-specific build directory
    let rootfs_path = format!("build/rootfs-alpine-{}", get_current_arch());

    // Path to supervisor binary - adjust this path as needed
    let supervisor_path = "../target/release/monokrun";

    // Create orchestrator with log retention policy
    let mut orchestrator = Orchestrator::with_log_retention_policy(
        rootfs_path,
        supervisor_path,
        LogRetentionPolicy::with_max_age_weeks(1),
    )
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

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------

// Helper function to print service status
async fn print_service_status(orchestrator: &Orchestrator) -> anyhow::Result<()> {
    println!("\nCurrent Service Status:");
    println!();
    let statuses = orchestrator.status().await?;

    println!(
        "{:<15} {:<10} {:<10} {:<15} {:<15} {:<12} {:<14}",
        "Service", "Group", "PID", "Status", "IP Address", "CPU Usage", "Memory Usage"
    );
    println!("{:-<92}", "");

    for status in statuses {
        println!(
            "{:<15} {:<10} {:<10} {:<15} {:<15} {:<12} {:<14}",
            status.get_name(),
            status.get_state().get_group().get_name(),
            status.get_pid().unwrap_or(0),
            format!("{:?}", status.get_state().get_status()),
            status
                .get_state()
                .get_group_ip()
                .map_or_else(|| Ipv4Addr::LOCALHOST, |ip| ip),
            status.get_state().get_metrics().get_cpu_usage(),
            status.get_state().get_metrics().get_memory_usage()
        );
    }
    println!();
    Ok(())
}

// Create initial configuration with two services
fn create_initial_config() -> anyhow::Result<Monocore> {
    // Create the main group
    let main_group = Group::builder().name("main").build();

    // Create initial services
    let tail_service = Service::builder_default()
        .name("tail-service")
        .base("alpine:latest")
        .group("main")
        .command("/usr/bin/tail")
        .args(["-f", "/dev/null"])
        .depends_on(["sleep-service".to_string()])
        .build();

    let sleep_service = Service::builder_default()
        .name("sleep-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sleep")
        .args(["infinity"])
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
    let main_group = Group::builder().name("main").build();

    // Create modified tail service (changed args)
    let tail_service = Service::builder_default()
        .name("tail-service")
        .base("alpine:latest")
        .group("main")
        .command("/usr/bin/tail")
        .args(["-f", "/etc/hosts"]) // Changed from /dev/null to /etc/hosts
        .build();

    // Keep sleep service unchanged
    let sleep_service = Service::builder_default()
        .name("sleep-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sleep")
        .args(["infinity"])
        .build();

    // Add new echo service
    let echo_service = Service::builder_default()
        .name("echo-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sh")
        .args([
            "-c",
            "while true; do echo 'Hello from echo service'; sleep 5; done",
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

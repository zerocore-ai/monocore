//! If you are trying to run this example, please make sure to run `make example orchestration_load` from
//! the `monocore` subdirectory.
//!
//! This example demonstrates the Orchestrator's ability to load and manage existing services:
//! - Starting some initial services with one Orchestrator instance
//! - Loading those running services into a new Orchestrator instance
//! - Showing that services continue running across Orchestrator instances
//!
//! To run the example:
//! ```bash
//! make example orchestration_load
//! ```
//!
//! The example will:
//! 1. Create an initial Orchestrator and start some services
//! 2. Drop the initial Orchestrator (simulating process restart)
//! 3. Load a new Orchestrator from the running services
//! 4. Show status of loaded services
//! 5. Clean up all services

use monocore::{
    config::{Group, Monocore, Service},
    orchestration::{LogRetentionPolicy, Orchestrator},
};
use std::{net::Ipv4Addr, time::Duration};
use tokio::time;
use tracing::info;

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
    let supervisor_path = "../target/release/monokrun";

    // Phase 1: Start initial services with first Orchestrator
    info!("Phase 1: Starting initial services with first Orchestrator");
    {
        let mut orchestrator = Orchestrator::new(&rootfs_path, supervisor_path).await?;
        let initial_config = create_services_config()?;

        orchestrator.up(initial_config).await?;

        // Wait for services to start
        time::sleep(Duration::from_secs(5)).await;
        info!("Initial services status:");
        print_service_status(&orchestrator).await?;

        info!("Dropping initial Orchestrator (simulating process restart)");
        // Orchestrator is dropped here, but services keep running
    }

    // Phase 2: Load running services into new Orchestrator
    info!("\nPhase 2: Loading existing services into new Orchestrator");
    let mut loaded_orchestrator = Orchestrator::load_with_log_retention_policy(
        &rootfs_path,
        supervisor_path,
        LogRetentionPolicy::with_max_age_days(7),
    )
    .await?;

    // Show that services are still running in loaded orchestrator
    info!("Services loaded from existing state:");
    print_service_status(&loaded_orchestrator).await?;

    // Wait a bit to show services are still running
    time::sleep(Duration::from_secs(5)).await;
    info!("\nServices still running after 5 seconds:");
    print_service_status(&loaded_orchestrator).await?;

    // Clean up
    info!("\nCleaning up services");
    loaded_orchestrator.down(None).await?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

// Helper function to print service status
async fn print_service_status(orchestrator: &Orchestrator) -> anyhow::Result<()> {
    let statuses = orchestrator.status().await?;

    println!("\nService Status:");
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

        // Format CPU as percentage
        let cpu_pct = (*status.get_state().get_metrics().get_cpu_usage() * 100.0).ceil();
        // Format memory in MiB (1 MiB = 1024 * 1024 bytes)
        let mem_mib =
            (*status.get_state().get_metrics().get_memory_usage() as f64) / (1024.0 * 1024.0);

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
                .map_or_else(|| Ipv4Addr::LOCALHOST, |ip| ip),
            format!("{}%", cpu_pct as u64),
            format!("{}MiB", mem_mib.ceil() as u64)
        );
    }
    println!();
    Ok(())
}

// Create configuration with some long-running services
fn create_services_config() -> anyhow::Result<Monocore> {
    let main_group = Group::builder().name("main").build();

    // Create services that will keep running
    let counter_service = Service::builder_default()
        .name("counter")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sh")
        .args([
            "-c",
            "i=0; while true; do echo Count: $i; i=$((i+1)); sleep 5; done",
        ])
        .build();

    let date_service = Service::builder_default()
        .name("date-service")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sh")
        .args(["-c", "while true; do date; sleep 3; done"])
        .build();

    let uptime_service = Service::builder_default()
        .name("uptime")
        .base("alpine:latest")
        .group("main")
        .command("/bin/sh")
        .args(["-c", "while true; do uptime; sleep 4; done"])
        .build();

    let config = Monocore::builder()
        .services(vec![counter_service, date_service, uptime_service])
        .groups(vec![main_group])
        .build()?;

    Ok(config)
}

fn get_current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        panic!("Unsupported architecture")
    }
}

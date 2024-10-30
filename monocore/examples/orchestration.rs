//! If you are trying to run this example, please make sure to run `make example runtime_lifecycle` from
//! the `monocore` subdirectory

use monocore::{
    config::{Group, Monocore, Service},
    orchestration::Orchestrator,
};
use std::net::Ipv4Addr;
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

    // Create an orchestrator instance with supervisor path
    let mut orchestrator = Orchestrator::new(config, rootfs_path, supervisor_path).await?;

    // Start all services
    println!("Starting all services...");
    orchestrator.up(None).await?;

    // Wait for a moment to let services start
    time::sleep(time::Duration::from_secs(2)).await;

    // Get status of all services
    println!("\nChecking service status...");
    let statuses = orchestrator.status().await?;
    for status in statuses {
        println!(
            "Service: {}, PID: {:?}, State: {:?}",
            status.name,
            status.pid,
            status.state.get_status()
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
    let main_group = Group::builder()
        .name("main".to_string())
        .address(Ipv4Addr::new(172, 0, 0, 1).into())
        .build();

    // Create a service that runs 'tail -f /dev/null' command (keeps running indefinitely)
    let tail_service = Service::builder_default()
        .name("tail-service")
        .base("alpine:latest")
        .group("main")
        .command("/usr/bin/tail")
        .argv(vec!["-f".to_string(), "/dev/null".to_string()])
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
        .build();

    Ok(config)
}

use anyhow::Result;
use monocore::{
    config::{Group, GroupEnv, GroupVolume, Monocore, Service, VolumeMount},
    orchestration::Orchestrator,
    utils,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Set up directories
    let build_dir = format!("{}/build", env!("CARGO_MANIFEST_DIR"));
    let oci_dir = format!("{}/oci", build_dir);
    let rootfs_dir = format!("{}/rootfs", build_dir);
    let rootfs_alpine_dir = format!("{}/reference/library_alpine__latest", rootfs_dir);

    // Pull and merge Alpine image
    utils::pull_docker_image(&oci_dir, "library/alpine:latest").await?;
    utils::merge_image_layers(&oci_dir, &rootfs_alpine_dir, "library/alpine:latest").await?;

    // Create a group configuration using builder
    let group = Group::builder()
        .name("grouped")
        .local_only(true)
        .volumes(vec![GroupVolume::builder()
            .name("ref_vols")
            .path("/Users/steveakinyemi/Desktop/Personal/test2")
            .build()])
        .envs(vec![GroupEnv::builder()
            .name("ref_envs")
            .envs(vec!["REFERENCE=steve".parse()?])
            .build()])
        .build();

    // Create a service configuration using builder
    let service = Service::builder()
        .name("example")
        .base("library/alpine:latest")
        .group("grouped")
        .command("/bin/sh")
        .args(vec![
            "-c".into(),
            "printenv; ls -la /test; ls -la /test2; ls -la /test3".into(),
        ])
        .cpus(1)
        .ram(256)
        .volumes(vec![
            "/Users/steveakinyemi/Desktop/Personal/test:/test".parse()?
        ])
        .envs(vec!["OWNED=steve".parse()?])
        .group_volumes(vec![VolumeMount::builder()
            .name("ref_vols")
            .mount("/Users/steveakinyemi/Desktop/Personal/test2:/test2".parse()?)
            .build()])
        .group_envs(vec!["ref_envs".into()])
        .build();

    // Create Monocore configuration
    let config = Monocore::builder()
        .services(vec![service])
        .groups(vec![group])
        .build()?;

    // Create and initialize the orchestrator
    let supervisor_path = "../target/release/monokrun";
    let mut orchestrator = Orchestrator::new(&build_dir, supervisor_path).await?;

    // Run the configuration
    orchestrator.up(config).await?;

    Ok(())
}

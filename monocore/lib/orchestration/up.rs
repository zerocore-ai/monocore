use super::Orchestrator;
use crate::{
    config::{Monocore, Service},
    oci::rootfs,
    utils::{
        MERGED_SUBDIR, OCI_REPO_SUBDIR, OCI_SUBDIR, REFERENCE_SUBDIR, ROOTFS_SUBDIR, SERVICE_SUBDIR,
    },
    MonocoreError, MonocoreResult,
};
use std::{collections::HashSet, net::Ipv4Addr, process::Stdio};
use tokio::{fs, process::Command};

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Orchestrator {
    /// Starts or updates services according to the provided configuration.
    /// Merges the new config with existing config and starts/restarts changed services.
    pub async fn up(&mut self, new_config: Monocore) -> MonocoreResult<()> {
        if self.log_retention_policy.auto_cleanup {
            if let Err(e) = self.cleanup_old_logs().await {
                tracing::warn!("Failed to clean up old logs during startup: {}", e);
            }
        }

        // Clone current config to avoid borrowing issues
        let current_config = self.config.clone();

        // Get the services that changed or were added
        let changed_services: HashSet<_> = current_config
            .get_changed_services(&new_config)
            .into_iter()
            .map(|s| s.get_name().to_string())
            .collect();

        // Merge the configurations
        self.config = current_config.merge(&new_config)?;

        // Get ordered list of changed services based on dependencies
        let ordered_services: Vec<_> = self
            .config
            .get_ordered_services()
            .into_iter()
            .filter(|s| changed_services.contains(s.get_name()))
            .collect();

        // Clone the ordered services to avoid borrow issues
        let ordered_services: Vec<_> = ordered_services.into_iter().cloned().collect();

        // Start/restart changed services in dependency order
        for service in ordered_services {
            // Stop the service if it's running
            if let Some(pid) = self.running_services.get(service.get_name()) {
                let pid = *pid; // Copy the pid to avoid borrow issues
                self.stop_service(pid).await?;
                self.running_services.remove(service.get_name());
            }

            // Start the service with new configuration
            self.start_service(&service).await?;
        }

        Ok(())
    }

    /// Starts a single service by spawning a supervisor process.
    pub(super) async fn start_service(&mut self, service: &Service) -> MonocoreResult<()> {
        if self.running_services.contains_key(service.get_name()) {
            tracing::info!("Service {} is already running", service.get_name());
            return Ok(());
        }

        // Get service-specific rootfs path
        let service_rootfs = self
            .home_dir
            .join(ROOTFS_SUBDIR)
            .join(SERVICE_SUBDIR)
            .join(service.get_name());

        // Get group and prepare configuration data
        let group = self
            .config
            .get_group_for_service(service.get_name())?
            .ok_or_else(|| {
                MonocoreError::ConfigValidation(format!(
                    "Service '{}' has no valid group configuration",
                    service.get_name()
                ))
            })?;
        let group_name = group.get_name().to_string();

        // Serialize configuration before IP assignment
        let service_json = serde_json::to_string(service)?;
        let group_json = serde_json::to_string(&group)?;

        // Create service directory and store service details
        self.store_service_details(service.get_name(), &service_json, &group_json)
            .await?;

        // If service rootfs doesn't exist, try to create it
        if !service_rootfs.exists() {
            tracing::info!(
                "Service rootfs not found at {}, attempting to create",
                service_rootfs.display()
            );

            // Get base image name from service config
            let base_image = service.get_base().ok_or_else(|| {
                MonocoreError::ConfigValidation(format!(
                    "Service {} has no base image specified",
                    service.get_name()
                ))
            })?;

            // Parse image reference
            let (_, _, repo_tag) = crate::utils::parse_image_ref(base_image)?;

            // Construct paths
            let reference_rootfs = self
                .home_dir
                .join(ROOTFS_SUBDIR)
                .join(REFERENCE_SUBDIR)
                .join(&repo_tag);

            let merged_rootfs = reference_rootfs.join(MERGED_SUBDIR);

            // First try using existing reference rootfs
            if merged_rootfs.exists() {
                tracing::info!(
                    "Using existing reference rootfs from {}",
                    merged_rootfs.display()
                );
                rootfs::copy(&merged_rootfs, &service_rootfs, false).await?;
            } else {
                // Check if image layers exist and try merging
                let repo_dir = self
                    .home_dir
                    .join(OCI_SUBDIR)
                    .join(OCI_REPO_SUBDIR)
                    .join(&repo_tag);

                if repo_dir.exists() {
                    tracing::info!(
                        "Reference rootfs not found but image layers exist, attempting merge for {}",
                        base_image
                    );

                    // Create parent directories
                    fs::create_dir_all(&reference_rootfs).await?;

                    // Merge layers into reference rootfs
                    rootfs::merge(
                        &self.home_dir.join(OCI_SUBDIR),
                        &reference_rootfs,
                        &repo_tag,
                    )
                    .await?;

                    // Copy merged rootfs to service rootfs
                    rootfs::copy(&merged_rootfs, &service_rootfs, false).await?;
                } else {
                    // Need to pull the image first
                    tracing::info!(
                        "Image layers not found for {}, attempting to pull image",
                        base_image
                    );

                    crate::utils::pull_docker_image(&self.home_dir.join(OCI_SUBDIR), base_image)
                        .await?;

                    // Create reference rootfs from pulled image
                    fs::create_dir_all(&reference_rootfs).await?;
                    rootfs::merge(
                        &self.home_dir.join(OCI_SUBDIR),
                        &reference_rootfs,
                        &repo_tag,
                    )
                    .await?;

                    // Copy to service rootfs
                    rootfs::copy(&merged_rootfs, &service_rootfs, false).await?;
                }
            }

            // Unmount reference rootfs
            rootfs::unmount(&reference_rootfs).await?;
        }

        // Assign IP address to the group
        let group_ip = self.assign_group_ip(&group_name)?;
        let group_ip_json = serde_json::to_string(&group_ip)?;

        // Start the supervisor process with service-specific rootfs
        let child = Command::new(&self.supervisor_exe_path)
            .arg("--run-supervisor")
            .args([
                &service_json,
                &group_json,
                &group_ip_json,
                service_rootfs.to_str().unwrap(),
                self.home_dir.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let pid = child
            .id()
            .ok_or_else(|| MonocoreError::ProcessIdNotFound(service.get_name().to_string()))?;

        self.running_services
            .insert(service.get_name().to_string(), pid);

        tracing::info!(
            "Started supervisor for service {} with PID {}",
            service.get_name(),
            pid
        );

        Ok(())
    }

    /// Assigns an IP address to a group from the 127.0.0.x range.
    /// Returns the existing IP if the group already has one assigned.
    ///
    /// The IP assignment follows these rules:
    /// - Uses addresses in the range 127.0.0.2 to 127.0.0.254
    /// - Skips 127.0.0.0, 127.0.0.1, and 127.0.0.255
    /// - Reuses IPs from terminated groups
    /// - Maintains consistent IP assignment for a group
    pub(super) fn assign_group_ip(&mut self, group_name: &str) -> MonocoreResult<Option<Ipv4Addr>> {
        // Return existing IP if already assigned
        if let Some(ip) = self.assigned_ips.get(group_name) {
            return Ok(Some(*ip));
        }

        // Find first available last octet (2-254, skipping 0, 1, and 255)
        let last_octet = match (2..=254).find(|&n| !self.used_ips.contains(&n)) {
            Some(n) => n,
            None => return Ok(None), // No IPs available
        };

        let ip = Ipv4Addr::new(127, 0, 0, last_octet);
        self.used_ips.insert(last_octet);
        self.assigned_ips.insert(group_name.to_string(), ip);

        Ok(Some(ip))
    }

    /// Stores service and group configuration files in the service directory.
    /// Only creates the files if they don't already exist.
    async fn store_service_details(
        &self,
        service_name: &str,
        service_json: &str,
        group_json: &str,
    ) -> MonocoreResult<()> {
        // Construct service directory path
        let service_dir = self.home_dir.join(SERVICE_SUBDIR).join(service_name);

        // Create service directory if it doesn't exist
        fs::create_dir_all(&service_dir).await?;

        // Paths to config files
        let service_json_path = service_dir.join("service.json");
        let group_json_path = service_dir.join("group.json");

        // Only write service.json if it doesn't exist
        if !service_json_path.exists() {
            fs::write(&service_json_path, service_json).await?;
            tracing::debug!(
                "Created service.json for {} at {}",
                service_name,
                service_json_path.display()
            );
        }

        // Only write group.json if it doesn't exist
        if !group_json_path.exists() {
            fs::write(&group_json_path, group_json).await?;
            tracing::debug!(
                "Created group.json for {} at {}",
                service_name,
                group_json_path.display()
            );
        }

        tracing::debug!(
            "Service details for {} stored in {}",
            service_name,
            service_dir.display()
        );

        Ok(())
    }
}

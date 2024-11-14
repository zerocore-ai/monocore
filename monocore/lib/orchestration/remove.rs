use crate::{
    utils::{ROOTFS_SUBDIR, SERVICE_SUBDIR},
    MonocoreError, MonocoreResult,
};
use serde::Deserialize;
use tokio::fs;

use super::Orchestrator;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ServiceConfig {
    group: Option<String>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Orchestrator {
    /// Removes the rootfs and configuration files for specified services.
    /// This only removes persistent files, not runtime state or logs.
    ///
    /// ## Arguments
    /// * `service_names` - Names of services to remove
    ///
    /// ## Example
    /// ```no_run
    /// # use monocore::orchestration::Orchestrator;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut orchestrator = Orchestrator::new("/path/to/rootfs", "/path/to/supervisor").await?;
    ///
    /// orchestrator.remove_services(&[
    ///     "service1".to_string(),
    ///     "service2".to_string()
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn remove_services(&mut self, service_names: &[String]) -> MonocoreResult<()> {
        if service_names.is_empty() {
            tracing::info!("No services specified for removal");
            return Ok(());
        }

        let service_dir = self.home_dir.join(SERVICE_SUBDIR);
        if !service_dir.exists() {
            return Ok(());
        }

        // Validate services exist and aren't running
        let mut services_to_remove = Vec::new();
        for name in service_names {
            let service_path = service_dir.join(name);
            if !service_path.exists() {
                tracing::warn!("Service directory not found: {}", service_path.display());
                continue;
            }

            if self.running_services.contains_key(name) {
                return Err(MonocoreError::ServiceStillRunning(format!(
                    "Cannot remove running service: {}",
                    name
                )));
            }

            services_to_remove.push(name);
        }

        for service_name in services_to_remove {
            self.remove_service_files(service_name).await?;
        }

        Ok(())
    }

    /// Removes all services belonging to the specified group.
    /// This only removes persistent files, not runtime state or logs.
    ///
    /// ## Arguments
    /// * `group_name` - Name of the group whose services should be removed
    ///
    /// ## Example
    /// ```no_run
    /// # use monocore::orchestration::Orchestrator;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut orchestrator = Orchestrator::new("/path/to/rootfs", "/path/to/supervisor").await?;
    ///
    /// orchestrator.remove_group("mygroup").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn remove_group(&mut self, group_name: &str) -> MonocoreResult<()> {
        let service_dir = self.home_dir.join(SERVICE_SUBDIR);
        if !service_dir.exists() {
            return Ok(());
        }

        let mut services_to_remove = Vec::new();
        let mut entries = fs::read_dir(&service_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let config_path = entry.path().join("service.json");
            if !config_path.exists() {
                continue;
            }

            // Read and parse service config to check group
            let config_str = fs::read_to_string(&config_path).await.map_err(|e| {
                MonocoreError::ConfigNotFound(format!(
                    "Failed to read service config at {}: {}",
                    config_path.display(),
                    e
                ))
            })?;
            let config: ServiceConfig = serde_json::from_str(&config_str).map_err(|e| {
                MonocoreError::ConfigNotFound(format!(
                    "Failed to parse service config at {}: {}",
                    config_path.display(),
                    e
                ))
            })?;

            if config.group.as_deref() == Some(group_name) {
                let service_name = entry.file_name().to_string_lossy().into_owned();

                // Check if service is running
                if self.running_services.contains_key(&service_name) {
                    return Err(MonocoreError::ServiceStillRunning(format!(
                        "Cannot remove running service: {}",
                        service_name
                    )));
                }

                services_to_remove.push(service_name);
            }
        }

        if services_to_remove.is_empty() {
            tracing::info!("No services found in group {}", group_name);
            return Ok(());
        }

        for service_name in services_to_remove {
            self.remove_service_files(&service_name).await?;
        }

        Ok(())
    }

    /// Helper method to remove service files (rootfs and config)
    async fn remove_service_files(&self, service_name: &str) -> MonocoreResult<()> {
        // Remove service rootfs
        let service_rootfs = self
            .home_dir
            .join(ROOTFS_SUBDIR)
            .join(SERVICE_SUBDIR)
            .join(service_name);
        if service_rootfs.exists() {
            fs::remove_dir_all(&service_rootfs).await.map_err(|e| {
                MonocoreError::LayerHandling {
                    source: e,
                    layer: service_rootfs.display().to_string(),
                }
            })?;
            tracing::info!("Removed service rootfs at {}", service_rootfs.display());
        }

        // Remove service configuration directory
        let service_config_dir = self.home_dir.join(SERVICE_SUBDIR).join(service_name);
        if service_config_dir.exists() {
            fs::remove_dir_all(&service_config_dir).await.map_err(|e| {
                MonocoreError::ConfigNotFound(format!(
                    "Failed to remove service config at {}: {}",
                    service_config_dir.display(),
                    e
                ))
            })?;
            tracing::info!("Removed service config at {}", service_config_dir.display());
        }

        Ok(())
    }
}

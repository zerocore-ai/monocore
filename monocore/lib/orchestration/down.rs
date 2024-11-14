use std::{collections::HashSet, time::Duration};

use tokio::time;

use crate::{orchestration::utils, MonocoreResult};

use super::Orchestrator;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Orchestrator {
    /// Stops running services and removes them from the configuration.
    /// When service_name is None, stops and removes all services.
    pub async fn down(&mut self, service_name: Option<&str>) -> MonocoreResult<()> {
        if self.log_retention_policy.auto_cleanup {
            if let Err(e) = self.cleanup_old_logs().await {
                tracing::warn!("Failed to clean up old logs during shutdown: {}", e);
            }
        }

        // Get the services to stop
        let services_to_stop: HashSet<String> = match service_name {
            Some(name) => vec![name.to_string()].into_iter().collect(),
            None => self.running_services.keys().cloned().collect(),
        };

        // Get all services in dependency order (reversed for shutdown)
        let ordered_services: Vec<_> = self
            .config
            .get_ordered_services()
            .into_iter()
            .filter(|s| services_to_stop.contains(s.get_name()))
            .rev() // Reverse the order for shutdown
            .collect();

        // Clone the ordered services to avoid borrow issues
        let ordered_services: Vec<_> = ordered_services.into_iter().cloned().collect();

        // Clone ordered_services before using it
        let services_for_groups = ordered_services.clone();

        // Stop services in reverse dependency order
        for service in ordered_services {
            let service_name = service.get_name();

            // Stop the service if it's running
            if let Some(pid) = self.running_services.remove(service_name) {
                tracing::info!(
                    "Stopping supervisor for service {} (PID {})",
                    service_name,
                    pid
                );

                if let Err(e) = self.stop_service(pid).await {
                    tracing::error!("Failed to send SIGTERM to service {}: {}", service_name, e);
                    continue;
                }

                // Wait for process to exit gracefully with timeout
                let mut attempts = 5;
                while attempts > 0 && utils::is_process_running(pid).await {
                    time::sleep(Duration::from_secs(2)).await;
                    attempts -= 1;
                }

                if utils::is_process_running(pid).await {
                    tracing::warn!(
                        "Service {} (PID {}) did not exit within timeout period",
                        service_name,
                        pid
                    );
                }
            }
        }

        // Convert HashSet back to Vec for remove_services
        let services_to_stop: Vec<_> = services_to_stop.into_iter().collect();

        // Remove services from config in place
        self.config.remove_services(Some(&services_to_stop));

        // Get groups that will have no running services after shutdown
        let mut empty_groups = HashSet::new();
        for service in services_for_groups.iter() {
            let group_name = service.get_group().unwrap_or_default();
            let group_has_other_services = self.running_services.keys().any(|name| {
                name != service.get_name()
                    && self
                        .config
                        .get_service(name)
                        .map(|s| s.get_group().unwrap_or_default() == group_name)
                        .unwrap_or(false)
            });
            if !group_has_other_services {
                empty_groups.insert(group_name);
            }
        }

        // Release IPs for groups with no running services
        for group_name in empty_groups {
            self.release_group_ip(group_name);
        }

        Ok(())
    }

    /// Releases an IP address assigned to a group, making it available for reuse.
    /// This should be called when a group no longer has any running services.
    pub(super) fn release_group_ip(&mut self, group_name: &str) {
        if let Some(ip) = self.assigned_ips.remove(group_name) {
            self.used_ips.remove(&ip.octets()[3]);
        }
    }
}

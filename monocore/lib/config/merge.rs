use std::collections::{HashMap, HashSet};

use super::Monocore;
use crate::{MonocoreError, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Monocore {
    /// Merges two Monocore configurations, producing a new configuration that combines both.
    /// This is used for updating running configurations while maintaining consistency.
    ///
    /// The merge follows these rules:
    /// - Services and groups from both configs are combined
    /// - If a service exists in both configs, the newer version (from other) takes precedence
    /// - If a group exists in both configs, the newer version takes precedence
    /// - Validates that the merged configuration maintains consistency and prevents impossible states
    /// - Ensures service dependencies remain valid after the merge
    /// - Prevents conflicts in resource allocation (ports, volumes, etc.)
    pub fn merge(&self, other: &Monocore) -> MonocoreResult<Monocore> {
        // Collect all service names for conflict checking
        let mut service_names: HashSet<String> = HashSet::new();
        let mut merged_services = Vec::new();

        // Merge services
        for service in &self.services {
            service_names.insert(service.get_name().to_string());
        }

        // Add services from the original config first
        merged_services.extend(self.services.clone());

        // Process services from the other config
        for service in &other.services {
            let service_name = service.get_name();

            // If service exists in both configs, replace the old version
            if service_names.contains(service_name) {
                merged_services.retain(|s| s.get_name() != service_name);
            }

            // Add new version
            merged_services.push(service.clone());
            service_names.insert(service_name.to_string());
        }

        // Merge groups with similar logic
        let mut group_names: HashSet<String> = HashSet::new();
        let mut merged_groups = Vec::new();

        // Add groups from original config
        for group in &self.groups {
            group_names.insert(group.get_name().clone());
            merged_groups.push(group.clone());
        }

        // Process groups from other config
        for group in &other.groups {
            let group_name = group.get_name();
            if group_names.contains(group_name) {
                // Replace existing group
                merged_groups.retain(|g| g.get_name() != group_name);
            }
            merged_groups.push(group.clone());
            group_names.insert(group_name.clone());
        }

        // Create merged configuration
        let merged = Monocore {
            services: merged_services,
            groups: merged_groups,
        };

        // Validate the merged configuration
        Self::validate_merged_config(&merged)?;

        Ok(merged)
    }

    /// Performs additional validation specific to merged configurations
    fn validate_merged_config(merged: &Monocore) -> MonocoreResult<()> {
        // Track volume mappings to prevent conflicts
        let mut volume_mappings: HashMap<String, HashSet<String>> = HashMap::new();

        // Track port usage per group
        let mut port_mappings: HashMap<Option<String>, HashMap<u16, String>> = HashMap::new();

        for service in &merged.services {
            // Check port conflicts within groups
            if let Some(port) = service.get_port() {
                let host_port = port.get_host();
                let group_ports = port_mappings
                    .entry(service.get_group().map(|g| g.to_string()))
                    .or_default();

                if let Some(existing_service) = group_ports.get(&host_port) {
                    return Err(MonocoreError::ConfigMerge(format!(
                        "Port {} is already in use by service '{}' in group '{}'",
                        host_port,
                        existing_service,
                        service.get_group().unwrap_or("default")
                    )));
                }
                group_ports.insert(host_port, service.get_name().to_string());
            }

            // Check volume conflicts
            for volume in service.get_volumes() {
                let host_path = volume.get_mount().get_host().to_string();
                let entry = volume_mappings.entry(host_path.clone()).or_default();

                if !entry.is_empty() && !entry.contains(service.get_name()) {
                    return Err(MonocoreError::ConfigMerge(format!(
                        "Volume path '{}' is mapped by multiple services",
                        host_path
                    )));
                }
                entry.insert(service.get_name().to_string());
            }

            // Validate service dependencies exist in merged config
            for dep in service.get_depends_on() {
                if !merged.services.iter().any(|s| s.get_name() == dep) {
                    return Err(MonocoreError::ConfigMerge(format!(
                        "Service '{}' depends on non-existent service '{}'",
                        service.get_name(),
                        dep
                    )));
                }
            }

            // Validate service group exists
            if let Some(group) = service.get_group() {
                if !merged.groups.iter().any(|g| g.get_name() == group) {
                    return Err(MonocoreError::ConfigMerge(format!(
                        "Service '{}' references non-existent group '{}'",
                        service.get_name(),
                        group
                    )));
                }
            }
        }

        // Check for circular dependencies in merged config
        if let Err(cycle) = merged.check_circular_dependencies() {
            return Err(MonocoreError::ConfigMerge(format!(
                "Merged configuration contains circular dependency: {}",
                cycle
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        monocore::{Group, Service},
        EnvPair, GroupEnv, GroupVolume, PortPair,
    };

    #[test]
    fn test_monocore_merge_basic() {
        // Create two services with different names
        let service1 = Service::builder_default()
            .name("service1")
            .command("./test1")
            .build();

        let service2 = Service::builder_default()
            .name("service2")
            .command("./test2")
            .build();

        // Create two valid configurations, each with one service
        let config1 = Monocore::builder()
            .services(vec![service1])
            .groups(vec![])
            .build()
            .unwrap();

        let config2 = Monocore::builder()
            .services(vec![service2])
            .groups(vec![])
            .build()
            .unwrap();

        // Merge should succeed and contain both services
        let merged = config1.merge(&config2).unwrap();
        assert_eq!(merged.services.len(), 2);
        assert!(merged.services.iter().any(|s| s.get_name() == "service1"));
        assert!(merged.services.iter().any(|s| s.get_name() == "service2"));
    }

    #[test]
    fn test_monocore_merge_service_update() {
        // Create original service
        let service1 = Service::builder_default()
            .name("service1")
            .command("./test1")
            .build();

        // Create updated version of the same service
        let service1_updated = Service::builder_default()
            .name("service1")
            .command("./test1_updated")
            .build();

        // Create configurations with original and updated services
        let config1 = Monocore::builder()
            .services(vec![service1])
            .groups(vec![])
            .build()
            .unwrap();

        let config2 = Monocore::builder()
            .services(vec![service1_updated])
            .groups(vec![])
            .build()
            .unwrap();

        // Merge should succeed and use the updated service
        let merged = config1.merge(&config2).unwrap();
        assert_eq!(merged.services.len(), 1);
        if let Service::Default { command, .. } = &merged.services[0] {
            assert_eq!(command.as_ref().unwrap(), "./test1_updated");
        }
    }

    #[test]
    fn test_monocore_merge_port_conflict() {
        // Create a group
        let group = Group::builder().name("test-group".to_string()).build();

        // Create two services in the same group that use the same port
        let service1 = Service::builder_default()
            .name("service1")
            .group("test-group")
            .port("8080:8080".parse::<PortPair>().unwrap())
            .command("./test1")
            .build();

        let service2 = Service::builder_default()
            .name("service2")
            .group("test-group")
            .port("8080:8080".parse::<PortPair>().unwrap())
            .command("./test2")
            .build();

        // Create configurations with conflicting port usage in same group
        let config1 = Monocore::builder()
            .services(vec![service1])
            .groups(vec![group.clone()])
            .build()
            .unwrap();

        // Create config2 without using builder to avoid validation
        let config2 = Monocore {
            services: vec![service2],
            groups: vec![group],
        };

        // Merge should succeed but validation should fail
        let result = config1.merge(&config2);
        println!("{:#?}", result);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Port 8080 is already in use"));
    }

    #[test]
    fn test_monocore_merge_port_different_groups() {
        // Create two groups
        let group1 = Group::builder().name("group1".to_string()).build();
        let group2 = Group::builder().name("group2".to_string()).build();

        // Create services in different groups using the same port
        let service1 = Service::builder_default()
            .name("service1")
            .group("group1")
            .port("8080:8080".parse::<PortPair>().unwrap())
            .command("./test1")
            .build();

        let service2 = Service::builder_default()
            .name("service2")
            .group("group2")
            .port("8080:8080".parse::<PortPair>().unwrap())
            .command("./test2")
            .build();

        // Create first config using builder
        let config1 = Monocore::builder()
            .services(vec![service1])
            .groups(vec![group1])
            .build()
            .unwrap();

        // Create second config without builder to avoid validation
        let config2 = Monocore {
            services: vec![service2],
            groups: vec![group2],
        };

        // Merge should succeed since services are in different groups
        let result = config1.merge(&config2);
        assert!(result.is_ok());
        let merged = result.unwrap();
        assert_eq!(merged.services.len(), 2);
        assert_eq!(merged.groups.len(), 2);
    }

    #[test]
    fn test_monocore_merge_groups() {
        // Create two groups with different names
        let group1 = Group::builder().name("group1".to_string()).build();
        let group2 = Group::builder().name("group2".to_string()).build();

        // Create configurations with different groups
        let config1 = Monocore::builder()
            .services(vec![])
            .groups(vec![group1])
            .build()
            .unwrap();

        let config2 = Monocore::builder()
            .services(vec![])
            .groups(vec![group2])
            .build()
            .unwrap();

        // Merge should succeed and contain both groups
        let merged = config1.merge(&config2).unwrap();
        assert_eq!(merged.groups.len(), 2);
    }

    #[test]
    fn test_monocore_merge_group_update() {
        // Create original group with a volume and env
        let group1 = Group::builder()
            .name("group1".to_string())
            .volumes(vec![GroupVolume::builder()
                .name("vol1".to_string())
                .path("/data".to_string())
                .build()])
            .envs(vec![GroupEnv::builder()
                .name("env1".to_string())
                .envs(vec![EnvPair::new("KEY1", "value1")])
                .build()])
            .build();

        // Create updated version of the same group with different volume and env
        let group1_updated = Group::builder()
            .name("group1".to_string())
            .volumes(vec![GroupVolume::builder()
                .name("vol1".to_string())
                .path("/data-updated".to_string())
                .build()])
            .envs(vec![GroupEnv::builder()
                .name("env1".to_string())
                .envs(vec![EnvPair::new("KEY1", "updated-value1")])
                .build()])
            .build();

        // Create configurations with original and updated groups
        let config1 = Monocore::builder()
            .services(vec![])
            .groups(vec![group1])
            .build()
            .unwrap();

        let config2 = Monocore::builder()
            .services(vec![])
            .groups(vec![group1_updated])
            .build()
            .unwrap();

        // Merge should succeed and use the updated group
        let merged = config1.merge(&config2).unwrap();
        assert_eq!(merged.groups.len(), 1);

        // Verify the group was actually updated
        let updated_group = &merged.groups[0];
        assert_eq!(updated_group.get_volumes()[0].get_path(), "/data-updated");
        assert_eq!(
            updated_group.get_envs()[0].get_envs()[0].get_value(),
            "updated-value1"
        );
    }

    #[test]
    fn test_monocore_merge_circular_dependency() {
        // Start with a valid configuration containing service1
        let service1 = Service::builder_default()
            .name("service1")
            .command("./test1")
            .build();

        let valid_config = Monocore::builder()
            .services(vec![service1])
            .groups(vec![])
            .build()
            .unwrap();

        // Try to merge with a new config that would create a circular dependency
        let service1_with_dep = Service::builder_default()
            .name("service1")
            .command("./test1")
            .depends_on(vec!["service2".to_string()])
            .build();

        let service2_with_dep = Service::builder_default()
            .name("service2")
            .command("./test2")
            .depends_on(vec!["service1".to_string()])
            .build();

        let update_config = Monocore {
            services: vec![service1_with_dep, service2_with_dep],
            groups: vec![],
        };

        // The merge should fail because it would create a circular dependency
        let result = valid_config.merge(&update_config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("circular dependency"));
    }
}

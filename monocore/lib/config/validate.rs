//! Monocore configuration validation

use std::collections::{HashMap, HashSet};

use typed_path::{Utf8UnixComponent, Utf8UnixPathBuf};

use crate::{utils, MonocoreError, MonocoreResult};

use super::monocore::{Monocore, Service};

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Monocore {
    /// Performs comprehensive validation of the Monocore configuration.
    /// This includes checking for:
    /// - Unique service and group names
    /// - Valid group references
    /// - Valid volume and environment references
    /// - Service dependencies
    /// - Service-specific configuration requirements
    /// - Circular dependencies in the service graph
    /// - Volume conflicts between groups
    /// - Port conflicts within groups
    pub fn validate(&self) -> MonocoreResult<()> {
        let mut errors = Vec::new();

        // Collect all service names and validate uniqueness
        let service_names = self.validate_service_names(&mut errors);

        // Collect all group names and validate uniqueness
        let group_names = self.validate_group_names(&mut errors);

        // Create mappings for validation
        let volume_map = self.build_volume_group_map();
        let env_map = self.build_env_group_map();

        // Validate services
        self.validate_services(
            &service_names,
            &group_names,
            &volume_map,
            &env_map,
            &mut errors,
        );

        // Check for circular dependencies
        if let Err(cycle) = self.check_circular_dependencies() {
            errors.push(format!("Circular dependency detected: {}", cycle));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(MonocoreError::ConfigValidationErrors(errors))
        }
    }

    /// Ensures service names are unique across the configuration.
    /// Collects all service names into a set while checking for duplicates.
    /// When duplicates are found, adds descriptive errors to the error list.
    fn validate_service_names(&self, errors: &mut Vec<String>) -> HashSet<&str> {
        let mut service_names = HashSet::new();

        for service in &self.services {
            let service_name = service.get_name();
            if !service_names.insert(service_name) {
                errors.push(format!("Duplicate service name '{}'", service_name));
            }
        }

        service_names
    }

    /// Ensures group names are unique across the configuration.
    /// Similar to service name validation, this collects group names while
    /// checking for duplicates and reporting any conflicts found.
    fn validate_group_names(&self, errors: &mut Vec<String>) -> HashSet<&str> {
        let mut group_names = HashSet::new();

        for group in &self.groups {
            let group_name = group.get_name();
            if !group_names.insert(group_name.as_str()) {
                errors.push(format!("Duplicate group name '{}'", group_name));
            }
        }

        group_names
    }

    /// Creates a mapping between volume names and their owning groups.
    /// This mapping is used to validate volume references and ensure volumes
    /// are accessed only from their assigned groups.
    fn build_volume_group_map(&self) -> HashMap<&str, &str> {
        self.groups
            .iter()
            .flat_map(|g| {
                g.get_volumes()
                    .iter()
                    .map(|v| (v.get_name().as_str(), g.get_name().as_str()))
            })
            .collect()
    }

    /// Creates a mapping between environment names and their owning groups.
    /// Similar to volume mapping, this is used to validate environment references
    /// and ensure environments are accessed only from their assigned groups.
    fn build_env_group_map(&self) -> HashMap<&str, &str> {
        self.groups
            .iter()
            .flat_map(|g| {
                g.get_envs()
                    .iter()
                    .map(|e| (e.get_name().as_str(), g.get_name().as_str()))
            })
            .collect()
    }

    /// Orchestrates the validation of all services in the configuration.
    /// Runs each service through the complete set of validation checks using
    /// the pre-collected validation data (names, mappings, etc).
    fn validate_services(
        &self,
        service_names: &HashSet<&str>,
        group_names: &HashSet<&str>,
        volume_map: &HashMap<&str, &str>,
        env_map: &HashMap<&str, &str>,
        errors: &mut Vec<String>,
    ) {
        self.validate_service_ports(&self.services, errors);
        self.validate_service_volumes(&self.services, errors);

        for service in &self.services {
            self.validate_service_declarations(service, errors);
            self.validate_service_group(service, group_names, errors);
            self.validate_service_group_volumes(service, volume_map, errors);
            self.validate_service_group_envs(service, env_map, errors);
            self.validate_service_dependencies(service, service_names, errors);
        }
    }

    /// Validates that services within the same group don't have port conflicts
    fn validate_service_ports(&self, services: &[Service], errors: &mut Vec<String>) {
        // Map of group name (or None for default group) to a map of host ports to service names
        let mut used_ports: HashMap<Option<String>, HashMap<u16, String>> = HashMap::new();

        for service in services {
            let group_ports = used_ports.entry(service.group.clone()).or_default();

            // Check each port in the service's ports vector
            for port in &service.ports {
                let host_port = port.get_host();

                if let Some(existing_service) = group_ports.get(&host_port) {
                    errors.push(format!(
                        "Port {} is already in use by service '{}' in group '{}'",
                        host_port,
                        existing_service,
                        service.group.as_deref().unwrap_or("default")
                    ));
                } else {
                    group_ports.insert(host_port, service.name.clone());
                }
            }
        }
    }

    /// Validates that a service's group reference points to an existing group.
    /// This ensures services don't reference non-existent groups in their configuration.
    fn validate_service_group(
        &self,
        service: &Service,
        group_names: &HashSet<&str>,
        errors: &mut Vec<String>,
    ) {
        if let Some(group) = &service.group {
            if !group_names.contains(group.as_str()) {
                errors.push(format!(
                    "Service '{}' references non-existent group '{}'",
                    service.name, group
                ));
            }
        }
    }

    /// Validates all volume references in a service configuration.
    /// Ensures that:
    /// - Referenced volumes exist
    /// - Volumes belong to the service's assigned group
    /// - Services don't access volumes from other groups
    fn validate_service_group_volumes(
        &self,
        service: &Service,
        volume_map: &HashMap<&str, &str>,
        errors: &mut Vec<String>,
    ) {
        let service_name = &service.name;

        for volume in service.get_group_volumes() {
            let volume_name = volume.get_name();
            match volume_map.get(volume_name.as_str()) {
                None => {
                    errors.push(format!(
                        "Service '{}' references non-existent volume '{}'",
                        service_name, volume_name
                    ));
                }
                Some(volume_group) => {
                    if let Some(service_group) = service.get_group() {
                        if service_group != *volume_group {
                            errors.push(format!(
                                "Service '{}' in group '{}' references volume '{}' from different group '{}'",
                                service_name, service_group, volume_name, volume_group
                            ));
                        }
                    }
                }
            }
        }
    }

    /// Validates all environment references in a service configuration.
    /// Similar to volume validation, this ensures:
    /// - Referenced environments exist
    /// - Environments belong to the service's assigned group
    /// - Services don't access environments from other groups
    fn validate_service_group_envs(
        &self,
        service: &Service,
        env_map: &HashMap<&str, &str>,
        errors: &mut Vec<String>,
    ) {
        let service_name = service.get_name();

        for env in service.get_group_envs() {
            match env_map.get(env.as_str()) {
                None => {
                    errors.push(format!(
                        "Service '{}' references non-existent env group '{}'",
                        service_name, env
                    ));
                }
                Some(env_group) => {
                    if let Some(service_group) = service.get_group() {
                        if service_group != *env_group {
                            errors.push(format!(
                                "Service '{}' in group '{}' references env group '{}' from different group '{}'",
                                service_name, service_group, env, env_group
                            ));
                        }
                    }
                }
            }
        }
    }

    /// Validates service dependencies to ensure they reference existing services.
    /// This prevents services from depending on non-existent services in their configuration.
    fn validate_service_dependencies(
        &self,
        service: &Service,
        service_names: &HashSet<&str>,
        errors: &mut Vec<String>,
    ) {
        let service_name = service.get_name();

        for dep in service.get_depends_on() {
            if !service_names.contains(dep.as_str()) {
                errors.push(format!(
                    "Service '{}' depends on non-existent service '{}'",
                    service_name, dep
                ));
            }
        }
    }

    /// Detects circular dependencies in the service dependency graph.
    /// A circular dependency occurs when services form a dependency cycle,
    /// which would prevent proper service startup ordering.
    pub fn check_circular_dependencies(&self) -> MonocoreResult<()> {
        // Build dependency graph as adjacency list
        let dep_graph = self.build_dependency_graph();

        // Check each service for cycles
        for service in &self.services {
            let service_name = service.get_name();
            let mut path = vec![service_name];
            let mut visited = HashSet::new();
            visited.insert(service_name);

            if let Some(cycle) =
                Self::find_cycle_from_service(service_name, &dep_graph, &mut visited, &mut path, 0)
            {
                return Err(MonocoreError::ConfigValidation(format!(
                    "Circular dependency detected: {}",
                    cycle.join(" -> ")
                )));
            }
        }

        Ok(())
    }

    /// Constructs a graph representation of service dependencies.
    /// Creates an adjacency list where each service maps to a list of its dependencies.
    /// This graph is used for circular dependency detection.
    fn build_dependency_graph(&self) -> HashMap<&str, Vec<&str>> {
        let mut graph = HashMap::new();

        for service in &self.services {
            graph.insert(
                service.get_name(),
                service.depends_on.iter().map(|s| s.as_str()).collect(),
            );
        }

        graph
    }

    /// Recursively searches for cycles in the dependency graph starting from a given service.
    /// Uses depth-first search with cycle detection to find any circular dependencies.
    /// Also enforces a maximum dependency chain length to prevent very deep recursion.
    fn find_cycle_from_service<'a>(
        current: &'a str,
        graph: &'a HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        path: &mut Vec<&'a str>,
        depth: usize,
    ) -> Option<Vec<&'a str>> {
        // Check maximum depth
        if depth >= Monocore::MAX_DEPENDENCY_DEPTH {
            return Some(path.clone()); // Consider deep chains as cycles
        }

        // Get dependencies for current service
        if let Some(deps) = graph.get(current) {
            for &dep in deps {
                // Found a cycle
                if path.contains(&dep) {
                    let mut cycle = path.clone();
                    cycle.push(dep);
                    return Some(cycle);
                }

                // Skip if already fully explored
                if visited.contains(&dep) {
                    continue;
                }

                // Explore this dependency
                visited.insert(dep);
                path.push(dep);

                if let Some(cycle) =
                    Self::find_cycle_from_service(dep, graph, visited, path, depth + 1)
                {
                    return Some(cycle);
                }

                path.pop();
            }
        }

        None
    }

    /// Validates that service declarations don't contain duplicates.
    /// This includes checking:
    /// - Environment references (both own and group)
    /// - Volume references (both own and group)
    /// - Dependencies
    fn validate_service_declarations(&self, service: &Service, errors: &mut Vec<String>) {
        let service_name = service.get_name();

        // Check for duplicate group environment references
        let mut env_names = HashSet::new();
        for env in service.get_group_envs() {
            if !env_names.insert(env) {
                errors.push(format!(
                    "Service '{}' has duplicate group environment reference '{}'",
                    service_name, env
                ));
            }
        }

        // Check for duplicate own environment references
        let mut own_env_names = HashSet::new();
        for env in service.get_own_envs() {
            let env_name = env.get_name();
            if !own_env_names.insert(env_name) {
                errors.push(format!(
                    "Service '{}' has duplicate own environment variable '{}'",
                    service_name, env_name
                ));
            }
        }

        // Check for duplicate group volume references
        let mut volume_names = HashSet::new();
        for volume in service.get_group_volumes() {
            if !volume_names.insert(volume.get_name()) {
                errors.push(format!(
                    "Service '{}' has duplicate group volume reference '{}'",
                    service_name,
                    volume.get_name()
                ));
            }
        }

        // Check for duplicate own volume references
        let mut own_volume_paths = HashSet::new();
        for volume in service.get_own_volumes() {
            let host_path = volume.get_host().to_string();
            if !own_volume_paths.insert(host_path.clone()) {
                errors.push(format!(
                    "Service '{}' has duplicate own volume path '{}'",
                    service_name, host_path
                ));
            }
        }

        // Check for duplicate dependencies
        let mut dep_names = HashSet::new();
        for dep in service.get_depends_on() {
            if !dep_names.insert(dep) {
                errors.push(format!(
                    "Service '{}' has duplicate dependency '{}'",
                    service_name, dep
                ));
            }
        }
    }

    /// Validates that volume paths don't conflict between different groups or services
    fn validate_service_volumes(&self, services: &[Service], errors: &mut Vec<String>) {
        // Collect all volume paths and their sources (group name or service name)
        let mut volume_paths: Vec<(String, String, bool)> = Vec::new(); // (path, source, is_group)

        // First collect group volume paths
        for group in &self.groups {
            let group_name = group.get_name();
            for volume in group.get_volumes() {
                let normalized_path = match normalize_path(volume.get_path(), true) {
                    Ok(path) => path,
                    Err(e) => {
                        errors.push(format!(
                            "Invalid volume path '{}' in group '{}': {}",
                            volume.get_path(),
                            group_name,
                            e
                        ));
                        continue;
                    }
                };
                volume_paths.push((normalized_path, group_name.to_string(), true));
            }
        }

        // Then collect service own volume paths
        for service in services {
            let service_name = service.get_name();
            for volume in service.get_own_volumes() {
                let host_path = volume.get_host();
                let normalized_path = match normalize_path(host_path.as_str(), true) {
                    Ok(path) => path,
                    Err(e) => {
                        errors.push(format!(
                            "Invalid volume path '{}' in service '{}': {}",
                            host_path, service_name, e
                        ));
                        continue;
                    }
                };
                volume_paths.push((normalized_path, service_name.to_string(), false));
            }

            // Validate service group volumes
            for volume_mount in service.get_group_volumes() {
                // Find the referenced group volume
                let volume_name = volume_mount.get_name();
                let group_name = match service.get_group() {
                    Some(group_name) => group_name.to_string(),
                    None => {
                        errors.push(format!(
                            "Service '{}' references group volume '{}' but has no group assigned",
                            service_name, volume_name
                        ));
                        continue;
                    }
                };

                // Find the group and its volume
                let group = match self.get_group(&group_name) {
                    Some(group) => group,
                    None => continue, // This error is already caught by group validation
                };

                let group_volume = match group
                    .get_volumes()
                    .iter()
                    .find(|v| v.get_name() == volume_name)
                {
                    Some(volume) => volume,
                    None => continue, // This error is already caught by group volume validation
                };

                // Normalize both the base path from the group volume and the requested mount path
                let base_path = group_volume.get_path();
                let mount_path = volume_mount.get_mount().get_host().to_string();

                match normalize_volume_path(base_path, &mount_path) {
                    Ok(normalized_path) => {
                        volume_paths.push((normalized_path, service_name.to_string(), false));
                    }
                    Err(e) => {
                        errors.push(format!(
                            "Invalid volume mount path '{}' for group volume '{}' in service '{}': {}",
                            mount_path, volume_name, service_name, e
                        ));
                    }
                }
            }
        }

        // Check for conflicts between all paths
        for i in 0..volume_paths.len() {
            let (path1, source1, is_group1) = &volume_paths[i];

            for (path2, source2, is_group2) in volume_paths.iter().skip(i + 1) {
                // Skip if both paths are from the same group or service
                if source1 == source2 {
                    continue;
                }

                // Get the group names for both sources
                let group1 = if *is_group1 {
                    Some(source1.as_str())
                } else {
                    self.get_service(source1).and_then(|s| s.get_group())
                };

                let group2 = if *is_group2 {
                    Some(source2.as_str())
                } else {
                    self.get_service(source2).and_then(|s| s.get_group())
                };

                // Skip if both sources belong to the same group
                if let (Some(g1), Some(g2)) = (group1, group2) {
                    if g1 == g2 {
                        continue;
                    }
                }

                // Check if one path is a prefix of the other (indicating a parent-child relationship)
                let is_conflict = utils::paths_overlap(path1, path2);

                if is_conflict {
                    let source1_type = if *is_group1 { "group" } else { "service" };
                    let source2_type = if *is_group2 { "group" } else { "service" };

                    errors.push(format!(
                        "Volume path conflict detected: path '{}' from {} '{}' conflicts with path '{}' from {} '{}'. \
                         Volume paths cannot overlap between different groups or services",
                        path1, source1_type, source1,
                        path2, source2_type, source2
                    ));
                }
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Normalizes a path string for volume mount comparison.
///
/// Rules:
/// - Resolves . and .. components where possible
/// - Prevents path traversal that would escape the root
/// - Removes redundant separators and trailing slashes
/// - Case-sensitive comparison (Unix standard)
/// - Can require absolute paths (for host mounts)
///
/// # Arguments
/// * `path` - The path to normalize
/// * `require_absolute` - If true, requires path to be absolute (start with '/')
///
/// # Returns
/// An error if the path is invalid, would escape root, or doesn't meet absolute requirement
pub fn normalize_path(path: &str, require_absolute: bool) -> MonocoreResult<String> {
    if path.is_empty() {
        return Err(MonocoreError::PathValidation(
            "Path cannot be empty".to_string(),
        ));
    }

    let path = Utf8UnixPathBuf::from(path);
    let mut normalized = Vec::new();
    let mut is_absolute = false;
    let mut depth = 0;

    for component in path.components() {
        match component {
            // Root component must come first if present
            Utf8UnixComponent::RootDir => {
                if normalized.is_empty() {
                    is_absolute = true;
                    normalized.push("/".to_string());
                } else {
                    return Err(MonocoreError::PathValidation(
                        "Invalid path: root component '/' found in middle of path".to_string(),
                    ));
                }
            }
            // Handle parent directory references
            Utf8UnixComponent::ParentDir => {
                if depth > 0 {
                    // Can go up if we have depth
                    normalized.pop();
                    depth -= 1;
                } else {
                    // Trying to go above root
                    return Err(MonocoreError::PathValidation(
                        "Invalid path: cannot traverse above root directory".to_string(),
                    ));
                }
            }
            // Skip current dir components
            Utf8UnixComponent::CurDir => continue,
            // Normal components are fine
            Utf8UnixComponent::Normal(c) => {
                if !c.is_empty() {
                    normalized.push(c.to_string());
                    depth += 1;
                }
            }
        }
    }

    // Check absolute path requirement if enabled
    if require_absolute && !is_absolute {
        return Err(MonocoreError::PathValidation(
            "Host mount paths must be absolute (start with '/')".to_string(),
        ));
    }

    if is_absolute {
        if normalized.len() == 1 {
            // Just root
            Ok("/".to_string())
        } else {
            // Join all components with "/" and add root at start
            Ok(format!("/{}", normalized[1..].join("/")))
        }
    } else {
        // For relative paths, just join all components
        Ok(normalized.join("/"))
    }
}

/// Helper function to normalize and validate volume paths
pub fn normalize_volume_path(base_path: &str, requested_path: &str) -> MonocoreResult<String> {
    // First normalize both paths
    let normalized_base = normalize_path(base_path, true)?;

    // If requested path is absolute, verify it's under base_path
    if requested_path.starts_with('/') {
        let normalized_requested = normalize_path(requested_path, true)?;
        // Check if normalized_requested starts with normalized_base
        if !normalized_requested.starts_with(&normalized_base) {
            return Err(MonocoreError::PathValidation(format!(
                "Absolute path '{}' must be under base path '{}'",
                normalized_requested, normalized_base
            )));
        }
        Ok(normalized_requested)
    } else {
        // For relative paths, first normalize the requested path to catch any ../ attempts
        let normalized_requested = normalize_path(requested_path, false)?;

        // Then join with base and normalize again
        let full_path = format!("{}/{}", normalized_base, normalized_requested);
        normalize_path(&full_path, true)
    }
}
//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        monocore::{Group, GroupEnv, GroupVolume},
        EnvPair, PathPair, PortPair, VolumeMount,
    };

    mod fixtures {
        use super::*;

        pub fn create_test_service(name: &str) -> Service {
            Service::builder().name(name).command("./test").build()
        }

        pub fn create_test_group(name: &str) -> Group {
            Group::builder()
                .name(name)
                .volumes(vec![])
                .envs(vec![])
                .local_only(true)
                .build()
        }
    }

    #[test]
    fn test_normalize_path() {
        // Test with require_absolute = true
        assert_eq!(normalize_path("/data/app/", true).unwrap(), "/data/app");
        assert_eq!(normalize_path("/data//app", true).unwrap(), "/data/app");
        assert_eq!(normalize_path("/data/./app", true).unwrap(), "/data/app");

        // Test with require_absolute = false
        assert_eq!(normalize_path("data/app/", false).unwrap(), "data/app");
        assert_eq!(normalize_path("./data/app", false).unwrap(), "data/app");
        assert_eq!(normalize_path("data//app", false).unwrap(), "data/app");

        // Path traversal within bounds
        assert_eq!(
            normalize_path("/data/temp/../app", true).unwrap(),
            "/data/app"
        );
        assert_eq!(
            normalize_path("data/temp/../app", false).unwrap(),
            "data/app"
        );

        // Invalid paths
        assert!(matches!(
            normalize_path("data/app", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("must be absolute")
        ));
        assert!(matches!(
            normalize_path("/data/../..", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
        assert!(matches!(
            normalize_path("data/../..", false),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
    }

    #[test]
    fn test_normalize_path_complex() {
        // Complex but valid paths
        assert_eq!(
            normalize_path("/data/./temp/../logs/app/./config/../", true).unwrap(),
            "/data/logs/app"
        );
        assert_eq!(
            normalize_path("/data///temp/././../app//./test/..", true).unwrap(),
            "/data/app"
        );

        // Edge cases
        assert_eq!(normalize_path("/data/./././.", true).unwrap(), "/data");
        assert_eq!(
            normalize_path("/data/test/../../data/app", true).unwrap(),
            "/data/app"
        );

        // Invalid complex paths
        assert!(matches!(
            normalize_path("/data/test/../../../root", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
        assert!(matches!(
            normalize_path("/./data/../..", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
    }

    #[test]
    fn test_monocore_validate_service_names_unique() {
        let mut monocore = Monocore {
            services: vec![
                fixtures::create_test_service("service1"),
                fixtures::create_test_service("service2"),
            ],
            groups: vec![],
        };

        let mut errors = Vec::new();
        let names = monocore.validate_service_names(&mut errors);

        assert_eq!(names.len(), 2);
        assert!(errors.is_empty());

        // Test duplicate names
        monocore = Monocore {
            services: vec![
                fixtures::create_test_service("service1"),
                fixtures::create_test_service("service1"),
            ],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_names(&mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Duplicate service name"));
    }

    #[test]
    fn test_monocore_validate_group_names_unique() {
        let mut monocore = Monocore {
            groups: vec![
                fixtures::create_test_group("group1"),
                fixtures::create_test_group("group2"),
            ],
            services: vec![],
        };

        let mut errors = Vec::new();
        let names = monocore.validate_group_names(&mut errors);

        assert_eq!(names.len(), 2);
        assert!(errors.is_empty());

        // Test duplicate names
        monocore = Monocore {
            groups: vec![
                fixtures::create_test_group("group1"),
                fixtures::create_test_group("group1"),
            ],
            services: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_group_names(&mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Duplicate group name"));
    }

    #[test]
    fn test_monocore_validate_service_group() {
        let service = Service::builder()
            .name("test-service")
            .group("test-group")
            .command("./test")
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![fixtures::create_test_group("test-group")],
        };

        let mut errors = Vec::new();
        let group_names = monocore.validate_group_names(&mut errors);
        monocore.validate_service_group(&monocore.services[0], &group_names, &mut errors);

        assert!(errors.is_empty());

        // Test non-existent group
        let service = Service::builder()
            .name("test-service")
            .group("non-existent")
            .command("./test")
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![fixtures::create_test_group("test-group")],
        };

        let mut errors = Vec::new();
        let group_names = monocore.validate_group_names(&mut errors);
        monocore.validate_service_group(&monocore.services[0], &group_names, &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("references non-existent group"));
    }

    #[test]
    fn test_monocore_validate_service_group_volumes() {
        let mut group = fixtures::create_test_group("test-group");
        group.volumes = vec![GroupVolume {
            name: "test-volume".to_string(),
            path: "/test".to_string(),
        }];

        let service = Service::builder()
            .name("test-service")
            .group("test-group")
            .command("./test")
            .group_volumes(vec![VolumeMount::builder()
                .name("test-volume")
                .mount("/test:/test".parse().unwrap())
                .build()])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![group],
        };

        let mut errors = Vec::new();
        let volume_map = monocore.build_volume_group_map();
        monocore.validate_service_group_volumes(&monocore.services[0], &volume_map, &mut errors);

        assert!(errors.is_empty());
    }

    #[test]
    fn test_monocore_validate_service_group_envs() {
        let mut group = fixtures::create_test_group("test-group");
        group.envs = vec![GroupEnv {
            name: "test-env".to_string(),
            envs: vec![EnvPair::new("TEST", "value")],
        }];

        let service = Service::builder()
            .name("test-service")
            .group("test-group")
            .command("./test")
            .group_envs(vec!["test-env".to_string()])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![group],
        };

        let mut errors = Vec::new();
        let env_map = monocore.build_env_group_map();
        monocore.validate_service_group_envs(&monocore.services[0], &env_map, &mut errors);

        assert!(errors.is_empty());
    }

    #[test]
    fn test_monocore_validate_service_dependencies() {
        let service1 = Service::builder()
            .name("service1")
            .command("./test1")
            .depends_on(vec!["service2".to_string()])
            .build();

        let service2 = Service::builder()
            .name("service2")
            .command("./test2")
            .build();

        let monocore = Monocore {
            services: vec![service1, service2],
            groups: vec![],
        };

        let mut errors = Vec::new();
        let service_names = monocore.validate_service_names(&mut errors);
        monocore.validate_service_dependencies(&monocore.services[0], &service_names, &mut errors);

        assert!(errors.is_empty());

        // Test non-existent dependency
        let service = Service::builder()
            .name("test-service")
            .command("./test")
            .depends_on(vec!["non-existent".to_string()])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        let service_names = monocore.validate_service_names(&mut errors);
        monocore.validate_service_dependencies(&monocore.services[0], &service_names, &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("depends on non-existent service"));
    }

    #[test]
    fn test_monocore_validate_service_declarations() {
        // Test duplicate group environment references
        let service = Service::builder()
            .name("test-service")
            .command("./test")
            .group_envs(vec!["env1".to_string(), "env1".to_string()])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate group environment reference"));

        // Test duplicate own environment variables
        let service = Service::builder()
            .name("test-service")
            .command("./test")
            .envs(vec![
                "TEST=value1".parse().unwrap(),
                "TEST=value2".parse().unwrap(),
            ])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate own environment variable"));

        // Test duplicate group volume references
        let service = Service::builder()
            .name("test-service")
            .command("./test")
            .group_volumes(vec![
                VolumeMount::builder()
                    .name("vol1")
                    .mount("/test:/test".parse().unwrap())
                    .build(),
                VolumeMount::builder()
                    .name("vol1")
                    .mount("/test2:/test2".parse().unwrap())
                    .build(),
            ])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate group volume reference"));

        // Test duplicate own volume paths
        let service = Service::builder()
            .name("test-service")
            .command("./test")
            .volumes(vec![
                "/data:/container1".parse().unwrap(),
                "/data:/container2".parse().unwrap(),
            ])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate own volume path"));

        // Test duplicate dependencies
        let service = Service::builder()
            .name("test-service")
            .command("./test")
            .depends_on(vec!["dep1".to_string(), "dep1".to_string()])
            .build();

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate dependency"));
    }

    #[test]
    fn test_validate_service_ports() {
        // Create services in the same group with conflicting ports
        let service1 = Service::builder()
            .name("service1")
            .group("test-group")
            .ports(vec![
                PortPair::with_same(8080),
                PortPair::with_distinct(8081, 81),
            ])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .group("test-group")
            .ports(vec![
                PortPair::with_same(8080),         // Conflicts with service1
                PortPair::with_distinct(8082, 82), // This one is fine
            ])
            .command("./test2")
            .build();

        let group = Group::builder().name("test-group").build();

        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![group],
        };

        let mut errors = Vec::new();
        config.validate_service_ports(&config.services, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Port 8080 is already in use"));
    }

    #[test]
    fn test_validate_service_ports_different_groups() {
        // Create services in different groups with same port
        let service1 = Service::builder()
            .name("service1")
            .group("group1")
            .ports(vec![
                PortPair::with_same(8080),
                PortPair::with_distinct(8081, 81),
            ])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .group("group2")
            .ports(vec![
                PortPair::with_same(8080), // Same port is fine in different group
                PortPair::with_distinct(8082, 82),
            ])
            .command("./test2")
            .build();

        let group1 = Group::builder().name("group1").build();
        let group2 = Group::builder().name("group2").build();

        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![group1, group2],
        };

        let mut errors = Vec::new();
        config.validate_service_ports(&config.services, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_service_ports_no_group() {
        // Create services with no group (default group) with conflicting ports
        let service1 = Service::builder()
            .name("service1")
            .ports(vec![
                PortPair::with_same(8080),
                PortPair::with_distinct(8081, 81),
            ])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .ports(vec![
                PortPair::with_same(8080),         // Conflicts with service1
                PortPair::with_distinct(8082, 82), // This one is fine
            ])
            .command("./test2")
            .build();

        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![],
        };

        let mut errors = Vec::new();
        config.validate_service_ports(&config.services, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Port 8080 is already in use"));
    }

    #[test]
    fn test_validate_service_ports_multiple_conflicts() {
        // Create services with multiple port conflicts
        let service1 = Service::builder()
            .name("service1")
            .group("test-group")
            .ports(vec![
                PortPair::with_same(8080),
                PortPair::with_same(8081),
                PortPair::with_distinct(8082, 82),
            ])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .group("test-group")
            .ports(vec![
                PortPair::with_same(8080),         // Conflicts with service1
                PortPair::with_same(8081),         // Also conflicts with service1
                PortPair::with_distinct(8083, 83), // This one is fine
            ])
            .command("./test2")
            .build();

        let group = Group::builder().name("test-group").build();

        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![group],
        };

        let mut errors = Vec::new();
        config.validate_service_ports(&config.services, &mut errors);
        assert_eq!(errors.len(), 2); // Should have two conflict errors
        assert!(errors
            .iter()
            .any(|e| e.contains("Port 8080 is already in use")));
        assert!(errors
            .iter()
            .any(|e| e.contains("Port 8081 is already in use")));
    }

    #[test]
    fn test_validate_service_ports_same_service() {
        // Create a service with duplicate ports
        let service = Service::builder()
            .name("service1")
            .ports(vec![
                PortPair::with_same(8080),
                PortPair::with_same(8080), // Duplicate port in same service
                PortPair::with_distinct(8081, 81),
            ])
            .command("./test")
            .build();

        let config = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        config.validate_service_ports(&config.services, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Port 8080 is already in use"));
    }

    #[test]
    fn test_monocore_validate_check_circular_dependencies() {
        // Create services with circular dependency
        let service1 = Service::builder()
            .name("service1")
            .command("./test1")
            .depends_on(vec!["service2".to_string()])
            .build();

        let service2 = Service::builder()
            .name("service2")
            .command("./test2")
            .depends_on(vec!["service3".to_string()])
            .build();

        let service3 = Service::builder()
            .name("service3")
            .command("./test3")
            .depends_on(vec!["service1".to_string()])
            .build();

        let monocore = Monocore {
            services: vec![service1, service2, service3],
            groups: vec![],
        };

        let result = monocore.check_circular_dependencies();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency detected"));
    }

    #[test]
    fn test_validate_service_volumes_cross_group_conflict() {
        // Create two groups
        let group1 = Group::builder().name("group1").build();
        let group2 = Group::builder().name("group2").build();

        // Create services in different groups trying to use the same volume path
        let service1 = Service::builder()
            .name("service1")
            .group("group1")
            .volumes(vec!["/data:/app".parse::<PathPair>().unwrap()])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .group("group2")
            .volumes(vec!["/data:/other".parse::<PathPair>().unwrap()])
            .command("./test2")
            .build();

        // Create configuration
        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![group1, group2],
        };

        // Validation should fail due to volume conflict
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("conflicts with path"));
    }

    #[test]
    fn test_validate_service_volumes_path_normalization() {
        // Create two groups
        let group1 = Group::builder().name("group1").build();
        let group2 = Group::builder().name("group2").build();

        // Create services using equivalent but differently formatted paths
        let service1 = Service::builder()
            .name("service1")
            .group("group1")
            .volumes(vec!["/data/app/".parse::<PathPair>().unwrap()])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .group("group2")
            .volumes(vec!["/data//app".parse::<PathPair>().unwrap()])
            .command("./test2")
            .build();

        // Create configuration
        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![group1, group2],
        };

        // Validation should fail since paths normalize to the same value
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("conflicts with path"));
    }

    #[test]
    fn test_validate_service_volumes_overlapping_paths() {
        // Create two groups
        let group1 = Group::builder().name("group1").build();
        let group2 = Group::builder().name("group2").build();

        // Test cases for different overlapping scenarios
        let test_cases = vec![
            // Case 1: Direct path overlap
            ("/data/app:/container", "/data/app:/other"),
            // Case 2: Parent-child relationship
            ("/data:/container", "/data/app:/other"),
            // Case 3: Child-parent relationship
            ("/data/app/logs:/container", "/data:/other"),
            // Case 4: Deeply nested overlap
            ("/data/apps/service1/logs:/container", "/data/apps:/other"),
            // Case 5: Path normalization cases
            ("/data/./app/logs:/container", "/data/app:/other"),
        ];

        for (path1, path2) in test_cases {
            // Create services in different groups with the test paths
            let service1 = Service::builder()
                .name("service1")
                .group("group1")
                .volumes(vec![path1.parse::<PathPair>().unwrap()])
                .command("./test1")
                .build();

            let service2 = Service::builder()
                .name("service2")
                .group("group2")
                .volumes(vec![path2.parse::<PathPair>().unwrap()])
                .command("./test2")
                .build();

            let config = Monocore {
                services: vec![service1, service2],
                groups: vec![group1.clone(), group2.clone()],
            };

            // Validation should fail for all test cases
            let result = config.validate();
            println!(">> result: {:?}", result);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("conflicts with path"));
        }

        // Test valid non-overlapping paths
        let service1 = Service::builder()
            .name("service1")
            .group("group1")
            .volumes(vec!["/data1/app:/container".parse::<PathPair>().unwrap()])
            .command("./test1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .group("group2")
            .volumes(vec!["/data2/app:/other".parse::<PathPair>().unwrap()])
            .command("./test2")
            .build();

        let config = Monocore {
            services: vec![service1, service2],
            groups: vec![group1, group2],
        };

        // Validation should succeed for non-overlapping paths
        let result = config.validate();
        assert!(
            result.is_ok(),
            "Non-overlapping paths should validate successfully"
        );
    }

    #[test]
    fn test_validate_service_volume_paths() {
        // Create a group with a volume
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![GroupVolume::builder()
                .name("data")
                .path("/data")
                .build()])
            .build();

        // Test 1: Invalid relative path in direct volume mount
        let service1 = Service::builder()
            .name("service1")
            .volumes(vec!["data/app:/app".parse::<PathPair>().unwrap()])
            .command("./test1")
            .build();

        let config1 = Monocore::builder()
            .services(vec![service1])
            .groups(vec![group.clone()])
            .build_unchecked();

        let result = config1.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Host mount paths must be absolute"));

        // Test 2: Invalid path traversal in direct volume mount
        let service2 = Service::builder()
            .name("service2")
            .volumes(vec!["/var/lib/../../../etc:/etc"
                .parse::<PathPair>()
                .unwrap()])
            .command("./test2")
            .build();

        let config2 = Monocore::builder()
            .services(vec![service2])
            .groups(vec![group.clone()])
            .build_unchecked();

        let result = config2.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot traverse above root"));

        // Test 3: Valid absolute path with normalization
        let service3 = Service::builder()
            .name("service3")
            .volumes(vec!["/var/./lib//app:/app".parse::<PathPair>().unwrap()])
            .command("./test3")
            .build();

        let config3 = Monocore::builder()
            .services(vec![service3])
            .groups(vec![group])
            .build_unchecked();

        let result = config3.validate();
        assert!(result.is_ok());
    }
}

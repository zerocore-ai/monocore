use std::collections::{HashMap, HashSet};

use crate::{MonocoreError, MonocoreResult};

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
            Err(MonocoreError::ConfigValidation(errors.join("\n")))
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

        for service in &self.services {
            self.validate_service_declarations(service, errors);
            self.validate_service_group(service, group_names, errors);
            self.validate_service_volumes(service, volume_map, errors);
            self.validate_service_envs(service, env_map, errors);
            self.validate_service_dependencies(service, service_names, errors);
            self.validate_service_specific_config(service, errors);
        }
    }

    /// Validates that services within the same group don't have port conflicts
    fn validate_service_ports(&self, services: &[Service], errors: &mut Vec<String>) {
        let mut used_ports: HashMap<Option<String>, HashMap<u16, String>> = HashMap::new();

        for service in services {
            if let Some(port) = service.get_port() {
                let host_port = port.get_host();
                let group_ports = used_ports
                    .entry(service.get_group().map(|g| g.to_string()))
                    .or_default();

                if let Some(existing_service) = group_ports.get(&host_port) {
                    errors.push(format!(
                        "Port {} is already in use by service '{}' in group '{}'",
                        host_port,
                        existing_service,
                        service.get_group().unwrap_or("default")
                    ));
                } else {
                    group_ports.insert(host_port, service.get_name().to_string());
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
        if let Some(group) = service.get_group() {
            if !group_names.contains(group) {
                errors.push(format!(
                    "Service '{}' references non-existent group '{}'",
                    service.get_name(),
                    group
                ));
            }
        }
    }

    /// Validates all volume references in a service configuration.
    /// Ensures that:
    /// - Referenced volumes exist
    /// - Volumes belong to the service's assigned group
    /// - Services don't access volumes from other groups
    fn validate_service_volumes(
        &self,
        service: &Service,
        volume_map: &HashMap<&str, &str>,
        errors: &mut Vec<String>,
    ) {
        let service_name = service.get_name();

        for volume in service.get_volumes() {
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
    fn validate_service_envs(
        &self,
        service: &Service,
        env_map: &HashMap<&str, &str>,
        errors: &mut Vec<String>,
    ) {
        let service_name = service.get_name();

        for env in service.get_envs() {
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

    /// Validates service-specific configuration requirements based on service type.
    /// For example:
    /// - Default services must have either a command or start script
    /// - HTTP handlers must specify a port
    fn validate_service_specific_config(&self, service: &Service, errors: &mut Vec<String>) {
        match service {
            Service::Default {
                command,
                scripts,
                name,
                ..
            } => {
                if command.is_none() && !scripts.contains_key("start") {
                    errors.push(format!(
                        "Service '{}' must specify either 'command' or 'scripts.start'",
                        name
                    ));
                }
            }
            Service::HttpHandler { port, name, .. } => {
                if port.is_none() {
                    errors.push(format!(
                        "HTTP handler service '{}' must specify a port",
                        name
                    ));
                }
            }
            Service::Precursor { .. } => {}
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
                service
                    .get_depends_on()
                    .iter()
                    .map(|s| s.as_str())
                    .collect(),
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
    /// - Environment references
    /// - Volume references
    /// - Dependencies
    /// - Script names
    fn validate_service_declarations(&self, service: &Service, errors: &mut Vec<String>) {
        let service_name = service.get_name();

        // Check for duplicate environment references
        let mut env_names = HashSet::new();
        for env in service.get_envs() {
            if !env_names.insert(env) {
                errors.push(format!(
                    "Service '{}' has duplicate environment reference '{}'",
                    service_name, env
                ));
            }
        }

        // Check for duplicate volume references
        let mut volume_names = HashSet::new();
        for volume in service.get_volumes() {
            if !volume_names.insert(volume.get_name()) {
                errors.push(format!(
                    "Service '{}' has duplicate volume reference '{}'",
                    service_name,
                    volume.get_name()
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
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        monocore::{Group, GroupEnv, GroupVolume, Service, ServiceVolume},
        EnvPair, PathPair, PortPair,
    };
    use std::collections::HashMap;

    mod fixtures {
        use super::*;

        pub fn create_test_service(name: &str) -> Service {
            Service::Default {
                name: name.to_string(),
                base: None,
                group: None,
                volumes: vec![],
                envs: vec![],
                depends_on: vec![],
                setup: vec![],
                scripts: HashMap::from([("start".to_string(), "./test".to_string())]),
                port: None,
                workdir: None,
                command: None,
                args: vec![],
                cpus: Monocore::default_num_vcpus(),
                ram: Monocore::default_ram_mib(),
            }
        }

        pub fn create_test_group(name: &str) -> Group {
            Group {
                name: name.to_string(),
                volumes: vec![],
                envs: vec![],
            }
        }
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
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { group, .. } = &mut service {
            *group = Some("test-group".to_string());
        }

        let monocore = Monocore {
            services: vec![service],
            groups: vec![fixtures::create_test_group("test-group")],
        };

        let mut errors = Vec::new();
        let group_names = monocore.validate_group_names(&mut errors);
        monocore.validate_service_group(&monocore.services[0], &group_names, &mut errors);

        assert!(errors.is_empty());

        // Test non-existent group
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { group, .. } = &mut service {
            *group = Some("non-existent".to_string());
        }

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
    fn test_monocore_validate_service_volumes() {
        let mut group = fixtures::create_test_group("test-group");
        group.volumes = vec![GroupVolume {
            name: "test-volume".to_string(),
            path: "/test".to_string(),
        }];

        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { volumes, group, .. } = &mut service {
            *volumes = vec![ServiceVolume {
                name: "test-volume".to_string(),
                mount: PathPair::Same("/test".parse().unwrap()),
            }];
            *group = Some("test-group".to_string());
        }

        let monocore = Monocore {
            services: vec![service],
            groups: vec![group],
        };

        let mut errors = Vec::new();
        let volume_map = monocore.build_volume_group_map();
        monocore.validate_service_volumes(&monocore.services[0], &volume_map, &mut errors);

        assert!(errors.is_empty());
    }

    #[test]
    fn test_monocore_validate_service_envs() {
        let mut group = fixtures::create_test_group("test-group");
        group.envs = vec![GroupEnv {
            name: "test-env".to_string(),
            envs: vec![EnvPair::new("TEST", "value")],
        }];

        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { envs, group, .. } = &mut service {
            *envs = vec!["test-env".to_string()];
            *group = Some("test-group".to_string());
        }

        let monocore = Monocore {
            services: vec![service],
            groups: vec![group],
        };

        let mut errors = Vec::new();
        let env_map = monocore.build_env_group_map();
        monocore.validate_service_envs(&monocore.services[0], &env_map, &mut errors);

        assert!(errors.is_empty());
    }

    #[test]
    fn test_monocore_validate_service_dependencies() {
        let mut service1 = fixtures::create_test_service("service1");
        if let Service::Default { depends_on, .. } = &mut service1 {
            *depends_on = vec!["service2".to_string()];
        }

        let service2 = fixtures::create_test_service("service2");

        let monocore = Monocore {
            services: vec![service1, service2],
            groups: vec![],
        };

        let mut errors = Vec::new();
        let service_names = monocore.validate_service_names(&mut errors);
        monocore.validate_service_dependencies(&monocore.services[0], &service_names, &mut errors);

        assert!(errors.is_empty());

        // Test non-existent dependency
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { depends_on, .. } = &mut service {
            *depends_on = vec!["non-existent".to_string()];
        }

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
    fn test_monocore_validate_service_specific_config() {
        // Test Default service without command or scripts.start
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default {
            scripts, command, ..
        } = &mut service
        {
            scripts.clear();
            *command = None;
        }

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_specific_config(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("must specify either 'command' or 'scripts.start'"));

        // Test HttpHandler service without port
        let service = Service::HttpHandler {
            name: "test-handler".to_string(),
            base: None,
            group: None,
            volumes: vec![],
            envs: vec![],
            depends_on: vec![],
            setup: vec![],
            port: None,
            cpus: Monocore::default_num_vcpus(),
            ram: Monocore::default_ram_mib(),
        };

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_specific_config(&monocore.services[0], &mut errors);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("must specify a port"));
    }

    #[test]
    fn test_monocore_validate_check_circular_dependencies() {
        // Create services with circular dependency
        let mut service1 = fixtures::create_test_service("service1");
        let mut service2 = fixtures::create_test_service("service2");
        let mut service3 = fixtures::create_test_service("service3");

        if let Service::Default { depends_on, .. } = &mut service1 {
            *depends_on = vec!["service2".to_string()];
        }
        if let Service::Default { depends_on, .. } = &mut service2 {
            *depends_on = vec!["service3".to_string()];
        }
        if let Service::Default { depends_on, .. } = &mut service3 {
            *depends_on = vec!["service1".to_string()];
        }

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
    fn test_monocore_validate_service_declarations() {
        // Test duplicate environment references
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { envs, .. } = &mut service {
            *envs = vec!["env1".to_string(), "env1".to_string()];
        }

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate environment reference"));

        // Test duplicate volume references
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { volumes, .. } = &mut service {
            *volumes = vec![
                ServiceVolume {
                    name: "vol1".to_string(),
                    mount: PathPair::Same("/test".parse().unwrap()),
                },
                ServiceVolume {
                    name: "vol1".to_string(),
                    mount: PathPair::Same("/test2".parse().unwrap()),
                },
            ];
        }

        let monocore = Monocore {
            services: vec![service],
            groups: vec![],
        };

        let mut errors = Vec::new();
        monocore.validate_service_declarations(&monocore.services[0], &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate volume reference"));

        // Test duplicate dependencies
        let mut service = fixtures::create_test_service("test-service");
        if let Service::Default { depends_on, .. } = &mut service {
            *depends_on = vec!["dep1".to_string(), "dep1".to_string()];
        }

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
        let service1 = Service::builder_default()
            .name("service1")
            .port("8080:8080".parse::<PortPair>().unwrap())
            .command("./test1")
            .build();

        let service2 = Service::builder_default()
            .name("service2")
            .port("8080:8080".parse::<PortPair>().unwrap())
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
}

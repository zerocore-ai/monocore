//! Monocore configuration types and helpers.

use std::collections::HashSet;

use getset::Getters;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

use crate::{MonocoreError, MonocoreResult};

use super::{
    monocore_builder::MonocoreBuilder,
    validate::{normalize_path, normalize_volume_path},
    EnvPair, PathPair, PortPair, ServiceBuilder, DEFAULT_NUM_VCPUS, DEFAULT_RAM_MIB,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The monocore configuration.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
pub struct Monocore {
    /// The services to run.
    #[serde(rename = "service")]
    pub(super) services: Vec<Service>,

    /// The groups to run the services in.
    #[serde(rename = "group", skip_serializing_if = "Vec::is_empty", default)]
    pub(super) groups: Vec<Group>,
}

/// The group to run the services in.
#[derive(Debug, Clone, Hash, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Group {
    /// The name of the group.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The volumes to mount.
    #[serde(rename = "volume", skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) volumes: Vec<GroupVolume>,

    /// The environment groups to use.
    #[serde(rename = "env", skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) envs: Vec<GroupEnv>,

    /// Whether services in this group are restricted to local connections only.
    #[serde(default = "Group::default_local_only")]
    #[builder(default = true)]
    pub(super) local_only: bool,
}

/// A volume definition in a group that specifies a base host path.
/// The path must be normalized (absolute path, no '..' components, no redundant separators).
/// Services in the group can mount this volume or its subdirectories using VolumeMount.
#[derive(Debug, Clone, Hash, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct GroupVolume {
    /// The name of the volume, used by services to reference this volume.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The normalized base path on the host system.
    /// Must be an absolute path without '..' components or redundant separators.
    #[builder(setter(transform = |path: impl AsRef<str>| path.as_ref().to_string()))]
    pub(super) path: String,
}

/// Specifies how a service mounts a group volume.
/// References a GroupVolume by name and specifies where to mount it in the guest.
#[derive(Debug, Clone, Serialize, TypedBuilder, Deserialize, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct VolumeMount {
    /// The name of the group volume to mount.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The mount specification.
    /// - If Same: mounts the group volume's path to the same path in guest
    /// - If Distinct: mounts the group volume's path to a specified guest path
    pub(super) mount: PathPair,
}

/// The environment group to use.
#[derive(Debug, Clone, Hash, Serialize, TypedBuilder, Deserialize, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct GroupEnv {
    /// The name of the environment group.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The environment variables to use.
    #[builder(default)]
    pub(super) envs: Vec<EnvPair>,
}

/// The service to run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Service {
    /// The name of the service.
    pub(super) name: String,

    /// The base image to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) base: Option<String>,

    /// The group to run the service in.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) group: Option<String>,

    /// The volumes specific to this service. These take precedence over group volumes.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(super) volumes: Vec<PathPair>,

    /// The environment variables specific to this service. These take precedence over group envs.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(super) envs: Vec<EnvPair>,

    /// The group volumes to use.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(super) group_volumes: Vec<VolumeMount>,

    /// The group environment variables to use.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(super) group_envs: Vec<String>,

    /// The services to depend on.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(super) depends_on: Vec<String>,

    /// The port to expose.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) port: Option<PortPair>,

    /// The working directory to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) workdir: Option<String>,

    /// The command to run. If the `scripts.start` is not specified, this will be used as the
    /// command to run.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) command: Option<String>,

    /// The arguments to pass to the command.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(super) args: Vec<String>,

    /// The number of vCPUs to use.
    #[serde(default = "Monocore::default_num_vcpus")]
    pub(super) cpus: u8,

    /// The amount of RAM in MiB to use.
    #[serde(default = "Monocore::default_ram_mib")]
    pub(super) ram: u32,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Monocore {
    /// The maximum service dependency chain length.
    pub const MAX_DEPENDENCY_DEPTH: usize = 32;

    /// Creates a new builder for a Monocore configuration.
    ///
    /// This builder provides a fluent interface for configuring and creating a Monocore configuration.
    /// The builder validates the configuration during build to ensure it is valid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use monocore::config::Monocore;
    ///
    /// let monocore = Monocore::builder()
    ///     .services(vec![])
    ///     .groups(vec![])
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> MonocoreBuilder {
        MonocoreBuilder::default()
    }

    /// Returns the default number of vCPUs.
    pub fn default_num_vcpus() -> u8 {
        DEFAULT_NUM_VCPUS
    }

    /// Returns the default amount of RAM in MiB.
    pub fn default_ram_mib() -> u32 {
        DEFAULT_RAM_MIB
    }

    /// Get a group by name in this configuration
    pub fn get_group(&self, group_name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == group_name)
    }

    /// Get all groups in this configuration
    pub fn get_groups(&self) -> &[Group] {
        &self.groups
    }

    /// Get a service by name in this configuration
    pub fn get_service(&self, service_name: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.get_name() == service_name)
    }

    /// Get all services in this configuration
    pub fn get_services(&self) -> &[Service] {
        &self.services
    }

    /// Removes specified services from the configuration in place.
    /// If service_names is None, removes all services.
    /// Groups are preserved unless all services are removed.
    ///
    /// ## Arguments
    /// * `names` - The set of service names to remove.
    pub fn remove_services(&mut self, names: &[String]) {
        self.services
            .retain(|s| !names.contains(&s.get_name().to_string()));
    }

    /// Gets all services ordered by their dependencies, such that dependencies come before dependents.
    /// This is useful for starting services in the correct order.
    ///
    /// ## Returns
    /// A vector of service references ordered by dependencies (dependencies first)
    ///
    /// ## Note
    /// This method assumes the configuration has already been validated (no circular dependencies).
    /// The validation is performed during configuration building.
    pub fn get_ordered_services(&self) -> Vec<&Service> {
        let mut ordered = Vec::new();
        let mut visited = HashSet::new();

        // Helper function for depth-first topological sort
        fn visit<'a>(
            service_name: &str,
            monocore: &'a Monocore,
            ordered: &mut Vec<&'a Service>,
            visited: &mut HashSet<String>,
        ) {
            // Skip if already visited
            if visited.contains(service_name) {
                return;
            }

            // Mark as visited
            visited.insert(service_name.to_string());

            // Get the service
            let service = monocore.get_service(service_name).unwrap(); // Safe because config is validated

            // Visit all dependencies first
            for dep in service.get_depends_on() {
                visit(dep, monocore, ordered, visited);
            }

            // Add service after its dependencies
            ordered.push(service);
        }

        // Visit all services
        for service in &self.services {
            visit(service.get_name(), self, &mut ordered, &mut visited);
        }

        ordered
    }

    /// Gets the group for a service by name.
    ///
    /// # Arguments
    /// * `service_name` - The name of the service to get the group for
    ///
    /// # Returns
    /// - `Ok(Some(group))` if the service exists and has a valid group configuration
    /// - `Ok(None)` if the service exists but:
    ///   - Has no group specified
    ///   - References a non-existent group
    /// - `Err(_)` if the service doesn't exist
    pub fn get_group_for_service<'a>(
        &'a self,
        service_name: &str,
    ) -> MonocoreResult<Option<&'a Group>> {
        let service = self.get_service(service_name).ok_or_else(|| {
            MonocoreError::ConfigValidation(format!("Service '{}' not found", service_name))
        })?;

        Ok(service
            .get_group()
            .and_then(|group_name| self.get_group(group_name)))
    }
}

impl Service {
    /// Creates a new builder for a service.
    pub fn builder() -> ServiceBuilder<()> {
        ServiceBuilder::default()
    }

    /// Gets the name of the service.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Gets the base image of the service.
    pub fn get_base(&self) -> Option<&str> {
        self.base.as_deref()
    }

    /// Gets the group of the service.
    ///
    /// ## Returns
    /// The name of the group the service is in, or None if the service is not in a group.
    pub fn get_group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Gets the services the service depends on.
    pub fn get_depends_on(&self) -> &[String] {
        &self.depends_on
    }

    /// Gets the port of the service.
    ///
    /// ## Returns
    /// The port of the service, or None if the service is not exposed.
    pub fn get_port(&self) -> Option<&PortPair> {
        self.port.as_ref()
    }

    /// Gets the working directory of the service.
    pub fn get_workdir(&self) -> Option<&str> {
        self.workdir.as_deref()
    }

    /// Gets the command of the service.
    pub fn get_command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// Gets the arguments of the service.
    pub fn get_args(&self) -> &[String] {
        &self.args
    }

    /// Gets the number of vCPUs the service uses.
    pub fn get_cpus(&self) -> u8 {
        self.cpus
    }

    /// Gets the amount of RAM in MiB the service uses.
    pub fn get_ram(&self) -> u32 {
        self.ram
    }

    /// Gets the environment variables specific to this service. These take precedence over group envs.
    pub fn get_own_envs(&self) -> &[EnvPair] {
        &self.envs
    }

    /// Gets the environment variables specific to this service's group.
    pub fn get_group_envs(&self) -> &[String] {
        &self.group_envs
    }

    /// Gets the volumes specific to this service. These take precedence over group volumes.
    pub fn get_own_volumes(&self) -> &[PathPair] {
        &self.volumes
    }

    /// Gets the volumes specific to this service's group.
    pub fn get_group_volumes(&self) -> &[VolumeMount] {
        &self.group_volumes
    }

    /// Resolves all environment variables for this service by merging group environment variables
    /// with service-specific ones. Service-specific variables take precedence over group variables.
    ///
    /// # Arguments
    /// * `group` - The group containing environment variable definitions
    ///
    /// # Returns
    /// A Result containing either:
    /// - Ok(Vec<EnvPair>): The merged environment variables
    /// - Err: If any referenced group environment doesn't exist
    pub fn resolve_environment_variables(&self, group: &Group) -> MonocoreResult<Vec<EnvPair>> {
        let mut env_pairs = Vec::new();

        // First add group environment variables
        for group_env_name in &self.group_envs {
            let group_env = group
                .get_envs()
                .iter()
                .find(|e| e.get_name() == group_env_name)
                .ok_or_else(|| {
                    MonocoreError::ConfigValidation(format!(
                        "Service '{}' references non-existent group environment '{}'",
                        self.name, group_env_name
                    ))
                })?;
            env_pairs.extend(group_env.get_envs().iter().cloned());
        }

        // Then add/override with service-specific environment variables
        for own_env in &self.envs {
            // Remove any existing env var with same name from group envs
            if let Some(idx) = env_pairs
                .iter()
                .position(|e| e.get_name() == own_env.get_name())
            {
                env_pairs.remove(idx);
            }
            env_pairs.push(own_env.clone());
        }

        Ok(env_pairs)
    }

    /// Resolves all volume mounts for this service by merging group volumes
    /// with service-specific ones. Service-specific volumes take precedence over group volumes
    /// when mounting to the same guest path.
    ///
    /// For group volumes:
    /// - Base path comes from group volume definition (must be normalized)
    /// - Service can specify a subdirectory of the base path to mount
    /// - Final host path will be base_path + service_subdir
    ///
    /// For service volumes:
    /// - Host paths are normalized
    /// - Direct mapping to guest path
    ///
    /// # Arguments
    /// * `group` - The group containing volume definitions
    ///
    /// # Returns
    /// A Result containing either:
    /// - Ok(Vec<PathPair>): The resolved volume mounts
    /// - Err: If any referenced group volume doesn't exist or if path normalization fails
    pub fn resolve_volumes(&self, group: &Group) -> MonocoreResult<Vec<PathPair>> {
        let mut volume_mounts = Vec::new();

        // First add group volumes referenced by the service
        for group_volume_mount in &self.group_volumes {
            let group_volume = group
                .get_volumes()
                .iter()
                .find(|v| v.get_name() == group_volume_mount.get_name())
                .ok_or_else(|| {
                    MonocoreError::ConfigValidation(format!(
                        "Service '{}' references non-existent group volume '{}'",
                        self.name,
                        group_volume_mount.get_name()
                    ))
                })?;

            // Group volume base path.
            let base_path = group_volume.get_path();

            // Create PathPair from group volume path and mount point
            let path_pair = match group_volume_mount.get_mount() {
                PathPair::Same(path) => {
                    let normalized_full_host_path =
                        normalize_volume_path(base_path, path.as_str())?;
                    PathPair::Distinct {
                        host: normalized_full_host_path.into(),
                        guest: path.into(),
                    }
                }
                PathPair::Distinct { host, guest } => {
                    let normalized_full_host_path =
                        normalize_volume_path(base_path, host.as_str())?;
                    PathPair::Distinct {
                        host: normalized_full_host_path.into(),
                        guest: guest.clone(),
                    }
                }
            };
            volume_mounts.push(path_pair);
        }

        // Then add/override with service-specific volumes
        for own_volume in &self.volumes {
            let normalized_volume = match own_volume {
                PathPair::Same(path) => {
                    let normalized = normalize_path(path.as_str(), true)?;
                    PathPair::Same(normalized.into())
                }
                PathPair::Distinct { host, guest } => {
                    let normalized = normalize_path(host.as_str(), true)?;
                    PathPair::Distinct {
                        host: normalized.into(),
                        guest: guest.clone(),
                    }
                }
            };

            // Remove any existing mount with same guest path
            if let Some(idx) = volume_mounts
                .iter()
                .position(|v| v.get_guest() == normalized_volume.get_guest())
            {
                volume_mounts.remove(idx);
            }
            volume_mounts.push(normalized_volume);
        }

        Ok(volume_mounts)
    }
}

impl Group {
    /// Returns the default value for local_only.
    pub fn default_local_only() -> bool {
        true
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for Service {
    fn default() -> Self {
        Service::builder().name(Uuid::new_v4().to_string()).build()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monocore_config_from_toml_string() -> anyhow::Result<()> {
        let config = r#"
        [[service]]
        name = "database"
        base = "postgres:16.1"
        volumes = [
            "/var/lib/postgresql/data:/"
        ]
        port = "5432:5432"

        [[service]]
        name = "server"
        base = "debian:12-slim"
        volumes = [
            "/logs:/"
        ]
        envs = [
            "LOG_LEVEL=info"
        ]
        group = "app_grp"
        group_envs = ["app_grp_env"]
        depends_on = ["database"]
        port = "3000:3000"
        command = "/app/bin/mcp-server"

        [[service.group_volumes]]
        name = "app_grp_vol"
        mount = "/User/mark/Desktop/project/server:/app"

        [[group]]
        name = "app_grp"
        local_only = true

        [[group.volume]]
        name = "app_grp_vol"
        path = "/User/mark/Desktop/project"

        [[group.env]]
        name = "app_grp_env"
        envs = [
            "PROJECT_PATH=/app"
        ]
        "#;

        let config: Monocore = toml::from_str(config)?;

        let expected_monocore = Monocore::builder()
            .services(vec![
                Service::builder()
                    .name("database")
                    .base("postgres:16.1")
                    .volumes(vec!["/var/lib/postgresql/data:/".parse::<PathPair>()?])
                    .port("5432:5432".parse::<PortPair>()?)
                    .build(),
                Service::builder()
                    .name("server")
                    .base("debian:12-slim")
                    .volumes(vec!["/logs:/".parse::<PathPair>()?])
                    .envs(vec!["LOG_LEVEL=info".parse::<EnvPair>()?])
                    .group("app_grp")
                    .group_envs(vec!["app_grp_env".to_string()])
                    .depends_on(vec!["database".to_string()])
                    .port("3000:3000".parse::<PortPair>()?)
                    .command("/app/bin/mcp-server")
                    .group_volumes(vec![VolumeMount::builder()
                        .name("app_grp_vol")
                        .mount("/User/mark/Desktop/project/server:/app".parse::<PathPair>()?)
                        .build()])
                    .build(),
            ])
            .groups(vec![Group::builder()
                .name("app_grp")
                .local_only(true)
                .volumes(vec![GroupVolume::builder()
                    .name("app_grp_vol")
                    .path("/User/mark/Desktop/project")
                    .build()])
                .envs(vec![GroupEnv::builder()
                    .name("app_grp_env")
                    .envs(vec!["PROJECT_PATH=/app".parse::<EnvPair>()?])
                    .build()])
                .build()])
            .build()?;

        assert_eq!(config, expected_monocore);

        Ok(())
    }

    #[test]
    fn test_monocore_config_from_json_string() -> anyhow::Result<()> {
        let config = r#"{
            "service": [
                {
                    "name": "database",
                    "base": "postgres:16.1",
                    "volumes": ["/var/lib/postgresql/data:/"],
                    "port": "5432:5432"
                },
                {
                    "name": "server",
                    "base": "debian:12-slim",
                    "volumes": ["/logs:/"],
                    "envs": ["LOG_LEVEL=info"],
                    "group": "app_grp",
                    "group_envs": ["app_grp_env"],
                    "depends_on": ["database"],
                    "port": "3000:3000",
                    "command": "/app/bin/mcp-server",
                    "group_volumes": [
                        {
                            "name": "app_grp_vol",
                            "mount": "/User/mark/Desktop/project/server:/app"
                        }
                    ]
                }
            ],
            "group": [
                {
                    "name": "app_grp",
                    "local_only": true,
                    "volume": [
                        {
                            "name": "app_grp_vol",
                            "path": "/User/mark/Desktop/project"
                        }
                    ],
                    "env": [
                        {
                            "name": "app_grp_env",
                            "envs": ["PROJECT_PATH=/app"]
                        }
                    ]
                }
            ]
        }"#;

        let config: Monocore = serde_json::from_str(config)?;

        let expected_monocore = Monocore::builder()
            .services(vec![
                Service::builder()
                    .name("database")
                    .base("postgres:16.1")
                    .volumes(vec!["/var/lib/postgresql/data:/".parse::<PathPair>()?])
                    .port("5432:5432".parse::<PortPair>()?)
                    .build(),
                Service::builder()
                    .name("server")
                    .base("debian:12-slim")
                    .volumes(vec!["/logs:/".parse::<PathPair>()?])
                    .envs(vec!["LOG_LEVEL=info".parse::<EnvPair>()?])
                    .group("app_grp")
                    .group_envs(vec!["app_grp_env".to_string()])
                    .depends_on(vec!["database".to_string()])
                    .port("3000:3000".parse::<PortPair>()?)
                    .command("/app/bin/mcp-server")
                    .group_volumes(vec![VolumeMount::builder()
                        .name("app_grp_vol")
                        .mount("/User/mark/Desktop/project/server:/app".parse::<PathPair>()?)
                        .build()])
                    .build(),
            ])
            .groups(vec![Group::builder()
                .name("app_grp")
                .local_only(true)
                .volumes(vec![GroupVolume::builder()
                    .name("app_grp_vol")
                    .path("/User/mark/Desktop/project")
                    .build()])
                .envs(vec![GroupEnv::builder()
                    .name("app_grp_env")
                    .envs(vec!["PROJECT_PATH=/app".parse::<EnvPair>()?])
                    .build()])
                .build()])
            .build()?;

        assert_eq!(config, expected_monocore);

        Ok(())
    }

    #[test]
    fn test_get_ordered_services() -> anyhow::Result<()> {
        // Create services with dependencies
        let service1 = Service::builder()
            .name("service1")
            .command("./service1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .command("./service2")
            .depends_on(vec!["service1".to_string()])
            .build();

        let service3 = Service::builder()
            .name("service3")
            .command("./service3")
            .depends_on(vec!["service2".to_string()])
            .build();

        // Create Monocore config with services in reverse order
        let config = Monocore::builder()
            .services(vec![service3.clone(), service2.clone(), service1.clone()])
            .build()?;

        // Get ordered services
        let ordered = config.get_ordered_services();

        // Check order
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].get_name(), "service1");
        assert_eq!(ordered[1].get_name(), "service2");
        assert_eq!(ordered[2].get_name(), "service3");

        Ok(())
    }

    #[test]
    fn test_get_ordered_services_circular() -> anyhow::Result<()> {
        // Create services with circular dependencies
        let service1 = Service::builder()
            .name("service1")
            .command("./service1")
            .depends_on(vec!["service2".to_string()])
            .build();

        let service2 = Service::builder()
            .name("service2")
            .command("./service2")
            .depends_on(vec!["service1".to_string()])
            .build();

        // Create Monocore config - this should fail due to circular dependency
        let result = Monocore::builder()
            .services(vec![service1, service2])
            .build();

        // The builder should fail validation
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));

        Ok(())
    }

    #[test]
    fn test_get_ordered_services_complex() -> anyhow::Result<()> {
        // Create a more complex dependency graph
        let service1 = Service::builder()
            .name("service1")
            .command("./service1")
            .build();

        let service2 = Service::builder()
            .name("service2")
            .command("./service2")
            .depends_on(vec!["service1".to_string()])
            .build();

        let service3 = Service::builder()
            .name("service3")
            .command("./service3")
            .depends_on(vec!["service1".to_string()])
            .build();

        let service4 = Service::builder()
            .name("service4")
            .command("./service4")
            .depends_on(vec!["service2".to_string(), "service3".to_string()])
            .build();

        // Create Monocore config
        let config = Monocore::builder()
            .services(vec![service4, service3, service2, service1])
            .build()?;

        // Get ordered services
        let ordered = config.get_ordered_services();

        // Check order
        assert_eq!(ordered.len(), 4);
        assert_eq!(ordered[0].get_name(), "service1");
        // service2 and service3 can be in either order since they both depend only on service1
        assert!(
            (ordered[1].get_name() == "service2" && ordered[2].get_name() == "service3")
                || (ordered[1].get_name() == "service3" && ordered[2].get_name() == "service2")
        );
        assert_eq!(ordered[3].get_name(), "service4");

        Ok(())
    }

    #[test]
    fn test_resolve_environment_variables() -> anyhow::Result<()> {
        // Create a group with some environment variables
        let group = Group::builder()
            .name("test-group")
            .envs(vec![GroupEnv::builder()
                .name("group_env1")
                .envs(vec![
                    "SHARED=from_group".parse()?,
                    "GROUP_ONLY=value".parse()?,
                ])
                .build()])
            .build();

        // Create a service that uses the group env and has its own env vars
        let service = Service::builder()
            .name("test-service")
            .group_envs(vec!["group_env1".to_string()])
            .envs(vec![
                "SHARED=from_service".parse()?, // Should override group value
                "SERVICE_ONLY=value".parse()?,
            ])
            .build();

        // Resolve environment variables
        let resolved = service.resolve_environment_variables(&group)?;

        // Check that we have the expected number of variables
        assert_eq!(resolved.len(), 3);

        // Check that service-specific value overrode group value
        assert!(resolved
            .iter()
            .any(|e| e.get_name() == "SHARED" && e.get_value() == "from_service"));

        // Check that other variables are present
        assert!(resolved
            .iter()
            .any(|e| e.get_name() == "GROUP_ONLY" && e.get_value() == "value"));
        assert!(resolved
            .iter()
            .any(|e| e.get_name() == "SERVICE_ONLY" && e.get_value() == "value"));

        Ok(())
    }

    #[test]
    fn test_resolve_volumes_with_normalization() -> anyhow::Result<()> {
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![GroupVolume::builder()
                .name("data")
                .path("/data/shared") // Base path
                .build()])
            .build();

        let service = Service::builder()
            .name("test-service")
            .group_volumes(vec![
                // Mount a subdirectory of the group volume with path that needs normalization
                VolumeMount::builder()
                    .name("data")
                    .mount("user1//subdir/:/container/data".parse()?) // Will become /data/shared/user1/subdir
                    .build(),
            ])
            .volumes(vec!["/var/log///app/:/container/logs".parse()?])
            .build();

        let resolved = service.resolve_volumes(&group)?;

        assert_eq!(resolved.len(), 2);

        // Check that combined group volume path is normalized
        assert!(resolved.iter().any(|v| {
            matches!(v, PathPair::Distinct { host, guest }
                if host == "/data/shared/user1/subdir" && guest == "/container/data")
        }));

        // Check that service-specific volume is normalized
        assert!(resolved.iter().any(|v| {
            matches!(v, PathPair::Distinct { host, guest }
                if host == "/var/log/app" && guest == "/container/logs")
        }));

        Ok(())
    }

    #[test]
    fn test_resolve_volumes_escape_prevention() -> anyhow::Result<()> {
        // Create a group with a base volume
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![GroupVolume::builder()
                .name("data")
                .path("/data/shared")
                .build()])
            .build();

        // Test 1: Direct path traversal attempt with relative path
        let service1 = Service::builder()
            .name("service1")
            .group_volumes(vec![VolumeMount::builder()
                .name("data")
                .mount("../escaped:/container/data".parse()?)
                .build()])
            .build();

        let result = service1.resolve_volumes(&group);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot traverse above root"));

        // Test 2: Sneaky path traversal with normalized result outside base
        let service2 = Service::builder()
            .name("service2")
            .group_volumes(vec![VolumeMount::builder()
                .name("data")
                .mount("subdir/../../etc:/container/data".parse()?)
                .build()])
            .build();

        let result = service2.resolve_volumes(&group);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot traverse above root"));

        // Test 3: Absolute path outside base path
        let service3 = Service::builder()
            .name("service3")
            .group_volumes(vec![VolumeMount::builder()
                .name("data")
                .mount("/etc/passwd:/container/data".parse()?)
                .build()])
            .build();

        let result = service3.resolve_volumes(&group);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be under base path"));

        // Test 4: Absolute path that is under base path
        let service4 = Service::builder()
            .name("service4")
            .group_volumes(vec![VolumeMount::builder()
                .name("data")
                .mount("/data/shared/logs:/container/data".parse()?)
                .build()])
            .build();

        let result = service4.resolve_volumes(&group)?;
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            PathPair::Distinct { host, guest }
            if host == "/data/shared/logs" && guest == "/container/data"
        ));

        // Test 5: Valid subdirectory mount
        let service5 = Service::builder()
            .name("service5")
            .group_volumes(vec![VolumeMount::builder()
                .name("data")
                .mount("subdir/logs:/container/data".parse()?)
                .build()])
            .build();

        let result = service5.resolve_volumes(&group)?;
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            PathPair::Distinct { host, guest }
            if host == "/data/shared/subdir/logs" && guest == "/container/data"
        ));

        Ok(())
    }

    #[test]
    fn test_resolve_environment_variables_missing_group_env() {
        let group = Group::builder()
            .name("test-group")
            .envs(vec![GroupEnv::builder()
                .name("existing-env")
                .envs(vec!["EXISTING=value".parse().unwrap()])
                .build()])
            .build();

        let service = Service::builder()
            .name("test-service")
            .group_envs(vec![
                "existing-env".to_string(),
                "non-existent-env".to_string(),
            ])
            .envs(vec!["SERVICE_ENV=value".parse().unwrap()])
            .build();

        let result = service.resolve_environment_variables(&group);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("references non-existent group environment"));
    }

    #[test]
    fn test_resolve_volumes_missing_group_volume() {
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![GroupVolume::builder()
                .name("existing-volume")
                .path("/data/existing")
                .build()])
            .build();

        let service = Service::builder()
            .name("test-service")
            .group_volumes(vec![
                VolumeMount::builder()
                    .name("existing-volume")
                    .mount("/data/existing:/app".parse().unwrap())
                    .build(),
                VolumeMount::builder()
                    .name("non-existent-volume")
                    .mount("/data/existing:/other".parse().unwrap())
                    .build(),
            ])
            .volumes(vec!["/service/data:/service".parse().unwrap()])
            .build();

        let result = service.resolve_volumes(&group);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("references non-existent group volume"));
    }
}

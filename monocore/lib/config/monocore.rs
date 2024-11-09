//! Monocore configuration types and helpers.

use std::collections::{HashMap, HashSet};

use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

use crate::{MonocoreError, MonocoreResult};

use super::{
    monocore_builder::MonocoreBuilder, EnvPair, PathPair, PortPair, ServiceDefaultBuilder,
    ServiceHttpHandlerBuilder, ServicePrecursorBuilder, DEFAULT_NUM_VCPUS, DEFAULT_RAM_MIB,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The monocore configuration.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Getters, Setters)]
#[getset(get = "pub with_prefix")]
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

impl Group {
    fn default_local_only() -> bool {
        true
    }
}

/// The volume to mount.
#[derive(Debug, Clone, Hash, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct GroupVolume {
    /// The name of the volume.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The path to mount the volume from.
    #[builder(setter(transform = |path: impl AsRef<str>| path.as_ref().to_string()))]
    pub(super) path: String,
}

/// The volume to mount.
#[derive(Debug, Clone, Serialize, TypedBuilder, Deserialize, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct ServiceVolume {
    /// The name of the volume.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The path to mount the volume to.
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
#[serde(tag = "type")]
pub enum Service {
    /// The default service.
    #[serde(rename = "default")]
    Default {
        /// The name of the service.
        name: String,

        /// The base image to use.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        base: Option<String>,

        /// The group to run the service in.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        group: Option<String>,

        /// The volumes to mount.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        volumes: Vec<ServiceVolume>,

        /// The environment groups to use.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        envs: Vec<String>,

        /// The services to depend on.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        depends_on: Vec<String>,

        /// The setup commands to run.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        setup: Vec<String>,

        /// The command to run.
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        scripts: HashMap<String, String>,

        /// The port to expose.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        port: Option<PortPair>,

        /// The working directory to use.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        workdir: Option<String>,

        /// The command to run. If the `scripts.start` is not specified, this will be used as the
        /// command to run.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        command: Option<String>,

        /// The arguments to pass to the command.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        args: Vec<String>,

        /// The number of vCPUs to use.
        #[serde(default = "Monocore::default_num_vcpus")]
        cpus: u8,

        /// The amount of RAM in MiB to use.
        #[serde(default = "Monocore::default_ram_mib")]
        ram: u32,
    },

    /// An HTTP event handler service. It enables serverless type workloads.
    #[serde(rename = "http_handler")]
    HttpHandler {
        /// The name of the service.
        name: String,

        /// The base image to use.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        base: Option<String>,

        /// The group to run the service in.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        group: Option<String>,

        /// The volumes to mount.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        volumes: Vec<ServiceVolume>,

        /// The environment groups to use.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        envs: Vec<String>,

        /// The services to depend on.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        depends_on: Vec<String>,

        /// The setup commands to run.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        setup: Vec<String>,

        /// The port to expose.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        port: Option<PortPair>,

        /// The number of vCPUs to use.
        #[serde(default = "Monocore::default_num_vcpus")]
        cpus: u8,

        /// The amount of RAM in MiB to use.
        #[serde(default = "Monocore::default_ram_mib")]
        ram: u32,
    },

    /// An ephemeral service that does not actually run anything.
    /// It is typically used to setup the environment for the actual services.
    #[serde(rename = "precursor")]
    Precursor {
        /// The name of the service.
        name: String,

        /// The base image to use.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        base: Option<String>,

        /// The volumes to mount.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        volumes: Vec<ServiceVolume>,

        /// The environment groups to use.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        envs: Vec<String>,

        /// The services to depend on.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        depends_on: Vec<String>,

        /// The setup commands to run.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        setup: Vec<String>,
    },
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
    pub fn get_group(&self, group_name: Option<&str>) -> Option<&Group> {
        group_name.and_then(|name| self.groups.iter().find(|g| g.get_name() == name))
    }

    /// Get a service by name in this configuration
    pub fn get_service(&self, service_name: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.get_name() == service_name)
    }
    /// Gets a group environment by name
    pub fn get_group_env(&self, env_name: &str, group_name: &str) -> Option<&GroupEnv> {
        // Find env in specified group
        self.groups
            .iter()
            .find(|g| g.get_name() == group_name)
            .and_then(|g| g.get_envs().iter().find(|e| e.get_name() == env_name))
    }

    /// Gets a group volume by name
    pub fn get_group_volume(&self, volume_name: &str, group_name: &str) -> Option<&GroupVolume> {
        self.groups
            .iter()
            .find(|g| g.get_name() == group_name)
            .and_then(|g| g.get_volumes().iter().find(|v| v.get_name() == volume_name))
    }

    /// Gets all environment variables for a service by combining all referenced env groups
    pub fn get_service_envs(&self, service: &Service) -> MonocoreResult<Vec<&EnvPair>> {
        let group_name = service.get_group().ok_or_else(|| {
            MonocoreError::ServiceBelongsToNoGroup(service.get_name().to_string())
        })?;

        Ok(service
            .get_envs()
            .iter()
            .filter_map(|env_name| self.get_group_env(env_name, group_name))
            .flat_map(|group_env| group_env.get_envs())
            .collect())
    }

    /// Gets all volumes for a service
    pub fn get_service_volumes<'a>(
        &'a self,
        service: &'a Service,
    ) -> MonocoreResult<Vec<(&'a GroupVolume, &'a ServiceVolume)>> {
        let group_name = service.get_group().ok_or_else(|| {
            MonocoreError::ServiceBelongsToNoGroup(service.get_name().to_string())
        })?;

        Ok(service
            .get_volumes()
            .iter()
            .filter_map(|service_volume| {
                self.get_group_volume(service_volume.get_name(), group_name)
                    .map(|group_volume| (group_volume, service_volume))
            })
            .collect())
    }

    /// Gets the group configuration for a service.
    pub fn get_group_for_service(&self, service: &Service) -> MonocoreResult<&Group> {
        let group_name = service.get_group().ok_or_else(|| {
            MonocoreError::ServiceBelongsToNoGroup(service.get_name().to_string())
        })?;

        self.groups
            .iter()
            .find(|g| g.get_name() == group_name)
            .ok_or_else(|| {
                MonocoreError::ConfigValidation(format!(
                    "Group not found for service {}: {}",
                    service.get_name(),
                    group_name
                ))
            })
    }

    /// Removes specified services from the configuration in place.
    /// If service_names is None, removes all services.
    /// Groups are preserved unless all services are removed.
    ///
    /// ## Arguments
    /// * `service_names` - Optional set of service names to remove. If None, removes all services.
    pub fn remove_services(&mut self, service_names: Option<&[String]>) {
        match service_names {
            Some(names) => {
                self.services
                    .retain(|s| !names.contains(&s.get_name().to_string()));
                if self.services.is_empty() {
                    self.groups.clear();
                }
            }
            None => {
                self.services.clear();
                self.groups.clear();
            }
        }
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
}

impl Service {
    /// Creates a new builder for a default service.
    ///
    /// This builder provides a fluent interface for configuring and creating a default service.
    /// Default services are general-purpose services that can run any command with custom configuration.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::config::Service;
    ///
    /// let service = Service::builder_default()
    ///     .name("my-service")
    ///     .base("ubuntu:24.04")
    ///     .group("app")
    ///     .build();
    /// ```
    pub fn builder_default() -> ServiceDefaultBuilder<()> {
        ServiceDefaultBuilder::default()
    }

    /// Creates a new builder for an HTTP handler service.
    ///
    /// This builder provides a fluent interface for configuring and creating an HTTP handler service.
    /// HTTP handler services are specialized services that handle HTTP requests in a serverless-like manner.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::config::Service;
    ///
    /// let service = Service::builder_http_handler()
    ///     .name("my-handler")
    ///     .base("ubuntu:24.04")
    ///     .port("8080:80".parse().unwrap())
    ///     .build();
    /// ```
    pub fn builder_http_handler() -> ServiceHttpHandlerBuilder<()> {
        ServiceHttpHandlerBuilder::default()
    }

    /// Creates a new builder for a precursor service.
    ///
    /// This builder provides a fluent interface for configuring and creating a precursor service.
    /// Precursor services are ephemeral services that run setup tasks before other services start.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use monocore::config::Service;
    ///
    /// let service = Service::builder_precursor()
    ///     .name("setup")
    ///     .base("ubuntu:24.04")
    ///     .setup(vec!["apt update".to_string()])
    ///     .build();
    /// ```
    pub fn builder_precursor() -> ServicePrecursorBuilder<()> {
        ServicePrecursorBuilder::default()
    }

    /// The default HTTP handler service.
    pub fn default_http_handler() -> Self {
        Service::HttpHandler {
            name: Uuid::new_v4().to_string(),
            base: None,
            group: None,
            volumes: vec![],
            envs: vec![],
            depends_on: vec![],
            setup: vec![],
            port: None,
            cpus: Monocore::default_num_vcpus(),
            ram: Monocore::default_ram_mib(),
        }
    }

    /// Gets all environment variables for a service by combining all referenced env groups
    pub fn get_group_env<'a>(&'a self, group: &'a Group) -> MonocoreResult<Vec<&'a EnvPair>> {
        // First check if service has a group
        let service_group = self
            .get_group()
            .ok_or_else(|| MonocoreError::ServiceBelongsToNoGroup(self.get_name().to_string()))?;

        // Then check if it matches the provided group
        if service_group != group.get_name() {
            return Err(MonocoreError::ServiceBelongsToWrongGroup(
                self.get_name().to_string(),
                group.get_name().to_string(),
            ));
        }

        // Get all environment variables from the referenced env groups
        Ok(self
            .get_envs()
            .iter()
            .filter_map(|env_name| {
                group
                    .get_envs()
                    .iter()
                    .find(|group_env| group_env.get_name() == env_name)
            })
            .flat_map(|group_env| group_env.get_envs())
            .collect())
    }

    /// Returns true if the service is a precursor.
    pub fn is_precursor(&self) -> bool {
        matches!(self, Service::Precursor { .. })
    }

    /// Returns true if the service is a default service.
    pub fn is_default(&self) -> bool {
        matches!(self, Service::Default { .. })
    }

    /// Returns true if the service is an HTTP handler service.
    pub fn is_http_handler(&self) -> bool {
        matches!(self, Service::HttpHandler { .. })
    }

    /// Returns the name of the service.
    pub fn get_name(&self) -> &str {
        match self {
            Service::Default { name, .. } => name,
            Service::Precursor { name, .. } => name,
            Service::HttpHandler { name, .. } => name,
        }
    }

    /// Returns the group of the service.
    pub fn get_group(&self) -> Option<&str> {
        match self {
            Service::Default { group, .. } => group.as_deref(),
            Service::Precursor { .. } => None,
            Service::HttpHandler { group, .. } => group.as_deref(),
        }
    }

    /// Returns the base image of the service.
    pub fn get_base(&self) -> Option<&str> {
        match self {
            Service::Default { base, .. } => base.as_deref(),
            Service::Precursor { base, .. } => base.as_deref(),
            Service::HttpHandler { base, .. } => base.as_deref(),
        }
    }

    /// Returns the volumes of the service.
    pub fn get_volumes(&self) -> &[ServiceVolume] {
        match self {
            Service::Default { volumes, .. } => volumes,
            Service::Precursor { volumes, .. } => volumes,
            Service::HttpHandler { volumes, .. } => volumes,
        }
    }

    /// Returns the environment groups to use.
    pub fn get_envs(&self) -> &[String] {
        match self {
            Service::Default { envs, .. } => envs,
            Service::Precursor { envs, .. } => envs,
            Service::HttpHandler { envs, .. } => envs,
        }
    }

    /// Returns the services to depend on.
    pub fn get_depends_on(&self) -> &[String] {
        match self {
            Service::Default { depends_on, .. } => depends_on,
            Service::Precursor { depends_on, .. } => depends_on,
            Service::HttpHandler { depends_on, .. } => depends_on,
        }
    }

    /// Returns the scripts of the service.
    pub fn get_scripts(&self) -> Option<&HashMap<String, String>> {
        match self {
            Service::Default { scripts, .. } => Some(scripts),
            _ => None,
        }
    }

    /// Returns the number of vCPUs to use.
    pub fn get_cpus(&self) -> u8 {
        match self {
            Service::Default { cpus, .. } => *cpus,
            Service::HttpHandler { cpus, .. } => *cpus,
            _ => Monocore::default_num_vcpus(),
        }
    }

    /// Returns the amount of RAM in MiB to use.
    pub fn get_ram(&self) -> u32 {
        match self {
            Service::Default { ram, .. } => *ram,
            Service::HttpHandler { ram, .. } => *ram,
            _ => Monocore::default_ram_mib(),
        }
    }

    /// Returns the port to expose.
    pub fn get_port(&self) -> Option<&PortPair> {
        match self {
            Service::Default { port, .. } => port.as_ref(),
            Service::HttpHandler { port, .. } => port.as_ref(),
            _ => None,
        }
    }

    /// Returns the working directory to use.
    pub fn get_workdir(&self) -> Option<&str> {
        match self {
            Service::Default { workdir, .. } => workdir.as_deref(),
            _ => None,
        }
    }

    /// Returns the command to run.
    pub fn get_command(&self) -> Option<&str> {
        match self {
            Service::Default { command, .. } => command.as_deref(),
            _ => None,
        }
    }

    /// Returns the arguments to pass to the command.
    pub fn get_args(&self) -> Option<&[String]> {
        match self {
            Service::Default { args, .. } => Some(args),
            _ => None,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for Service {
    fn default() -> Self {
        Service::Default {
            name: Uuid::new_v4().to_string(),
            base: None,
            group: None,
            volumes: vec![],
            envs: vec![],
            depends_on: vec![],
            setup: vec![],
            scripts: HashMap::new(),
            port: None,
            workdir: None,
            command: None,
            args: vec![],
            cpus: Monocore::default_num_vcpus(),
            ram: Monocore::default_ram_mib(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MonocoreError;

    #[test]
    fn test_monocore_config_from_toml_string() -> anyhow::Result<()> {
        let config = r#"
        [[service]]
        type = "precursor"
        name = "precursor"
        base = "ubuntu:24.04"
        envs = ["main"]
        setup = [
            "apt update && apt install -y curl",
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            "cd /project && cargo build --release",
            "cp target/release/monocore /main/monocore"
        ]

        [[service]]
        type = "default"
        name = "server"
        base = "ubuntu:24.04"
        group = "app"
        volumes = [
            { name = "main", mount = "/project:/" }
        ]
        envs = ["main"]
        depends_on = ["precursor"]
        setup = [
            "cd /main"
        ]
        port = "3000:3000"
        scripts = { start = "./monocore" }

        [[group]]
        name = "app"
        address = "10.0.0.1"
        local_only = true

        [[group.volume]]
        name = "main"
        path = "~/Desktop/project"

        [[group.env]]
        name = "main"
        envs = [
            "LOG_LEVEL=info",
            "PROJECT_PATH=/project"
        ]
        "#;

        let config: Monocore = toml::from_str(config)?;

        tracing::info!("config: {:?}", config);

        let mut scripts = HashMap::new();
        scripts.insert("start".to_string(), "./monocore".to_string());

        let expected_monocore = Monocore::builder()
            .services(vec![
                Service::builder_precursor()
                    .name("precursor")
                    .base("ubuntu:24.04")
                    .envs(vec!["main".to_string()])
                    .setup(vec![
                        "apt update && apt install -y curl".to_string(),
                        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
                            .to_string(),
                        "cd /project && cargo build --release".to_string(),
                        "cp target/release/monocore /main/monocore".to_string(),
                    ])
                    .build(),
                Service::builder_default()
                    .name("server")
                    .base("ubuntu:24.04")
                    .group("app")
                    .volumes(vec![ServiceVolume::builder()
                        .name("main")
                        .mount(PathPair::Distinct {
                            host: "/project".parse()?,
                            guest: "/".parse()?,
                        })
                        .build()])
                    .envs(vec!["main".to_string()])
                    .depends_on(vec!["precursor".to_string()])
                    .setup(vec!["cd /main".to_string()])
                    .scripts(scripts)
                    .port("3000:3000".parse()?)
                    .build(),
            ])
            .groups(vec![Group::builder()
                .name("app")
                .volumes(vec![GroupVolume::builder()
                    .name("main")
                    .path("~/Desktop/project")
                    .build()])
                .envs(vec![GroupEnv::builder()
                    .name("main")
                    .envs(vec![
                        "LOG_LEVEL=info".parse()?,
                        "PROJECT_PATH=/project".parse()?,
                    ])
                    .build()])
                .local_only(true)
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
                    "type": "precursor",
                    "name": "precursor",
                    "base": "ubuntu:24.04",
                    "envs": ["main"],
                    "setup": [
                        "apt update && apt install -y curl",
                        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
                        "cd /project && cargo build --release",
                        "cp target/release/monocore /main/monocore"
                    ]
                },
                {
                    "type": "default",
                    "name": "server",
                    "base": "ubuntu:24.04",
                    "group": "app",
                    "volumes": [
                        {
                            "name": "main",
                            "mount": "/project:/"
                        }
                    ],
                    "envs": ["main"],
                    "depends_on": ["precursor"],
                    "setup": ["cd /main"],
                    "port": "3000:3000",
                    "scripts": {
                        "start": "./monocore"
                    }
                }
            ],
            "group": [
                {
                    "name": "app",
                    "local_only": true,
                    "volume": [
                        {
                            "name": "main",
                            "path": "~/Desktop/project"
                        }
                    ],
                    "env": [
                        {
                            "name": "main",
                            "envs": [
                                "LOG_LEVEL=info",
                                "PROJECT_PATH=/project"
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let config: Monocore = serde_json::from_str(config)?;

        let mut scripts = std::collections::HashMap::new();
        scripts.insert("start".to_string(), "./monocore".to_string());

        let expected_monocore = Monocore::builder()
            .services(vec![
                Service::builder_precursor()
                    .name("precursor")
                    .base("ubuntu:24.04")
                    .envs(vec!["main".to_string()])
                    .setup(vec![
                        "apt update && apt install -y curl".to_string(),
                        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
                            .to_string(),
                        "cd /project && cargo build --release".to_string(),
                        "cp target/release/monocore /main/monocore".to_string(),
                    ])
                    .build(),
                Service::builder_default()
                    .name("server")
                    .base("ubuntu:24.04")
                    .group("app")
                    .volumes(vec![ServiceVolume::builder()
                        .name("main".to_string())
                        .mount(PathPair::Distinct {
                            host: "/project".parse()?,
                            guest: "/".parse()?,
                        })
                        .build()])
                    .envs(vec!["main".to_string()])
                    .depends_on(vec!["precursor".to_string()])
                    .setup(vec!["cd /main".to_string()])
                    .scripts(scripts)
                    .port("3000:3000".parse()?)
                    .build(),
            ])
            .groups(vec![Group::builder()
                .name("app")
                .volumes(vec![GroupVolume::builder()
                    .name("main")
                    .path("~/Desktop/project".to_string())
                    .build()])
                .envs(vec![GroupEnv::builder()
                    .name("main".to_string())
                    .envs(vec![
                        "LOG_LEVEL=info".parse()?,
                        "PROJECT_PATH=/project".parse()?,
                    ])
                    .build()])
                .local_only(true)
                .build()])
            .build()?;

        assert_eq!(config, expected_monocore);

        Ok(())
    }

    #[test]
    fn test_monocore_config_get_group_env() -> anyhow::Result<()> {
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![])
            .envs(vec![GroupEnv::builder()
                .name("test-env")
                .envs(vec![EnvPair::new("TEST", "value")])
                .build()])
            .build();

        let monocore = Monocore::builder()
            .services(vec![])
            .groups(vec![group])
            .build()?;

        // Test finding env in specific group
        let env = monocore.get_group_env("test-env", "test-group");
        assert!(env.is_some());
        assert_eq!(env.unwrap().get_name(), "test-env");

        // Test non-existent env in existing group
        let env = monocore.get_group_env("non-existent", "test-group");
        assert!(env.is_none());

        // Test env in non-existent group
        let env = monocore.get_group_env("test-env", "non-existent-group");
        assert!(env.is_none());

        Ok(())
    }

    #[test]
    fn test_monocore_config_get_group_volume() -> anyhow::Result<()> {
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![GroupVolume::builder()
                .name("test-volume")
                .path("/test")
                .build()])
            .envs(vec![])
            .build();

        let monocore = Monocore::builder()
            .services(vec![])
            .groups(vec![group])
            .build()?;

        // Test finding volume in specific group
        let volume = monocore.get_group_volume("test-volume", "test-group");
        assert!(volume.is_some());
        assert_eq!(volume.unwrap().get_name(), "test-volume");

        // Test non-existent volume in existing group
        let volume = monocore.get_group_volume("non-existent", "test-group");
        assert!(volume.is_none());

        // Test volume in non-existent group
        let volume = monocore.get_group_volume("test-volume", "non-existent-group");
        assert!(volume.is_none());

        Ok(())
    }

    #[test]
    fn test_monocore_config_get_service_envs() -> anyhow::Result<()> {
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![])
            .envs(vec![GroupEnv::builder()
                .name("test-env")
                .envs(vec![
                    EnvPair::new("TEST1", "value1"),
                    EnvPair::new("TEST2", "value2"),
                ])
                .build()])
            .build();

        let service = Service::builder_default()
            .name("test-service")
            .command("/bin/sleep")
            .group("test-group")
            .envs(vec!["test-env".to_string()])
            .build();

        let monocore = Monocore::builder()
            .services(vec![service])
            .groups(vec![group])
            .build()?;

        let envs = monocore.get_service_envs(&monocore.services[0])?;
        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].get_name(), "TEST1");
        assert_eq!(envs[0].get_value(), "value1");
        assert_eq!(envs[1].get_name(), "TEST2");
        assert_eq!(envs[1].get_value(), "value2");

        Ok(())
    }

    #[test]
    fn test_monocore_config_get_service_envs_no_group() -> anyhow::Result<()> {
        let service = Service::builder_default()
            .name("test-service")
            .command("/bin/sleep")
            .build();

        let monocore = Monocore::builder()
            .services(vec![service])
            .groups(vec![])
            .build()?;

        let result = monocore.get_service_envs(&monocore.services[0]);
        assert!(matches!(
            result,
            Err(MonocoreError::ServiceBelongsToNoGroup(name)) if name == "test-service"
        ));

        Ok(())
    }

    #[test]
    fn test_monocore_config_get_service_volumes() -> anyhow::Result<()> {
        let group = Group::builder()
            .name("test-group")
            .volumes(vec![GroupVolume::builder()
                .name("test-volume")
                .path("/test")
                .build()])
            .envs(vec![])
            .build();

        let service = Service::builder_default()
            .name("test-service")
            .command("/bin/sleep")
            .group("test-group")
            .volumes(vec![ServiceVolume::builder()
                .name("test-volume")
                .mount(PathPair::Same("/test".parse()?))
                .build()])
            .build();

        let monocore = Monocore::builder()
            .services(vec![service])
            .groups(vec![group])
            .build()?;

        let volumes = monocore.get_service_volumes(&monocore.services[0])?;
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].0.get_name(), "test-volume");
        assert_eq!(volumes[0].1.get_name(), "test-volume");

        Ok(())
    }

    #[test]
    fn test_monocore_config_get_service_volumes_no_group() -> anyhow::Result<()> {
        let service = Service::builder_default()
            .name("test-service")
            .command("/bin/sleep")
            .build();

        let monocore = Monocore::builder()
            .services(vec![service])
            .groups(vec![])
            .build()?;

        let result = monocore.get_service_volumes(&monocore.services[0]);
        assert!(matches!(
            result,
            Err(MonocoreError::ServiceBelongsToNoGroup(name)) if name == "test-service"
        ));

        Ok(())
    }

    #[test]
    fn test_get_ordered_services() -> anyhow::Result<()> {
        // Create services with dependencies
        let service1 = Service::builder_default()
            .name("service1")
            .command("./service1")
            .build();

        let service2 = Service::builder_default()
            .name("service2")
            .command("./service2")
            .depends_on(vec!["service1".to_string()])
            .build();

        let service3 = Service::builder_default()
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
        let service1 = Service::builder_default()
            .name("service1")
            .command("./service1")
            .depends_on(vec!["service2".to_string()])
            .build();

        let service2 = Service::builder_default()
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
        let service1 = Service::builder_default()
            .name("service1")
            .command("./service1")
            .build();

        let service2 = Service::builder_default()
            .name("service2")
            .command("./service2")
            .depends_on(vec!["service1".to_string()])
            .build();

        let service3 = Service::builder_default()
            .name("service3")
            .command("./service3")
            .depends_on(vec!["service1".to_string()])
            .build();

        let service4 = Service::builder_default()
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
}

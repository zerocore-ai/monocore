use std::collections::HashMap;

use crate::config::{
    monocore::{Monocore, Service, ServiceVolume},
    PortPair,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Builder for the Service::Default variant
pub struct ServiceDefaultBuilder<Name> {
    name: Name,
    base: Option<String>,
    group: Option<String>,
    volumes: Vec<ServiceVolume>,
    envs: Vec<String>,
    depends_on: Vec<String>,
    setup: Vec<String>,
    scripts: HashMap<String, String>,
    port: Option<PortPair>,
    workdir: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    cpus: u8,
    ram: u32,
}

/// Builder for the Service::HttpHandler variant
pub struct ServiceHttpHandlerBuilder<Name> {
    name: Name,
    base: Option<String>,
    group: Option<String>,
    volumes: Vec<ServiceVolume>,
    envs: Vec<String>,
    depends_on: Vec<String>,
    setup: Vec<String>,
    port: Option<PortPair>,
    cpus: u8,
    ram: u32,
}

/// Builder for the Service::Precursor variant
pub struct ServicePrecursorBuilder<Name> {
    name: Name,
    base: Option<String>,
    volumes: Vec<ServiceVolume>,
    envs: Vec<String>,
    depends_on: Vec<String>,
    setup: Vec<String>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<Name> ServiceDefaultBuilder<Name> {
    /// Sets the base image for the service
    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }

    /// Sets the group for the service
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Sets the volumes for the service
    pub fn volumes(mut self, volumes: impl IntoIterator<Item = ServiceVolume>) -> Self {
        self.volumes = volumes.into_iter().collect();
        self
    }

    /// Sets the environment groups for the service
    pub fn envs(mut self, envs: impl IntoIterator<Item = String>) -> Self {
        self.envs = envs.into_iter().collect();
        self
    }

    /// Sets the dependencies for the service
    pub fn depends_on(mut self, depends_on: impl IntoIterator<Item = String>) -> Self {
        self.depends_on = depends_on.into_iter().collect();
        self
    }

    /// Sets the setup commands for the service
    pub fn setup(mut self, setup: impl IntoIterator<Item = String>) -> Self {
        self.setup = setup.into_iter().collect();
        self
    }

    /// Sets the scripts for the service
    pub fn scripts(mut self, scripts: HashMap<String, String>) -> Self {
        self.scripts = scripts;
        self
    }

    /// Sets the port mapping for the service
    pub fn port(mut self, port: PortPair) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the working directory for the service
    pub fn workdir(mut self, workdir: impl Into<String>) -> Self {
        self.workdir = Some(workdir.into());
        self
    }

    /// Sets the command for the service
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Sets the command arguments for the service
    pub fn args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        self.args = args.into_iter().collect();
        self
    }

    /// Sets the number of CPUs for the service
    pub fn cpus(mut self, cpus: u8) -> Self {
        self.cpus = cpus;
        self
    }

    /// Sets the RAM amount for the service
    pub fn ram(mut self, ram: u32) -> Self {
        self.ram = ram;
        self
    }

    /// Sets the name for the service
    pub fn name(self, name: impl Into<String>) -> ServiceDefaultBuilder<String> {
        ServiceDefaultBuilder {
            name: name.into(),
            base: self.base,
            group: self.group,
            volumes: self.volumes,
            envs: self.envs,
            depends_on: self.depends_on,
            setup: self.setup,
            scripts: self.scripts,
            port: self.port,
            workdir: self.workdir,
            command: self.command,
            args: self.args,
            cpus: self.cpus,
            ram: self.ram,
        }
    }
}

impl ServiceDefaultBuilder<String> {
    /// Builds the Service::Default variant
    pub fn build(self) -> Service {
        Service::Default {
            name: self.name,
            base: self.base,
            group: self.group,
            volumes: self.volumes,
            envs: self.envs,
            depends_on: self.depends_on,
            setup: self.setup,
            scripts: self.scripts,
            port: self.port,
            workdir: self.workdir,
            command: self.command,
            args: self.args,
            cpus: self.cpus,
            ram: self.ram,
        }
    }
}

impl<Name> ServiceHttpHandlerBuilder<Name> {
    /// Sets the base image for the service
    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }

    /// Sets the group for the service
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Sets the volumes for the service
    pub fn volumes(mut self, volumes: impl IntoIterator<Item = ServiceVolume>) -> Self {
        self.volumes = volumes.into_iter().collect();
        self
    }

    /// Sets the environment groups for the service
    pub fn envs(mut self, envs: impl IntoIterator<Item = String>) -> Self {
        self.envs = envs.into_iter().collect();
        self
    }

    /// Sets the dependencies for the service
    pub fn depends_on(mut self, depends_on: impl IntoIterator<Item = String>) -> Self {
        self.depends_on = depends_on.into_iter().collect();
        self
    }

    /// Sets the setup commands for the service
    pub fn setup(mut self, setup: impl IntoIterator<Item = String>) -> Self {
        self.setup = setup.into_iter().collect();
        self
    }

    /// Sets the port mapping for the service
    pub fn port(mut self, port: PortPair) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the number of CPUs for the service
    pub fn cpus(mut self, cpus: u8) -> Self {
        self.cpus = cpus;
        self
    }

    /// Sets the RAM amount for the service
    pub fn ram(mut self, ram: u32) -> Self {
        self.ram = ram;
        self
    }

    /// Sets the name for the service
    pub fn name(self, name: impl Into<String>) -> ServiceHttpHandlerBuilder<String> {
        ServiceHttpHandlerBuilder {
            name: name.into(),
            base: self.base,
            group: self.group,
            volumes: self.volumes,
            envs: self.envs,
            depends_on: self.depends_on,
            setup: self.setup,
            port: self.port,
            cpus: self.cpus,
            ram: self.ram,
        }
    }
}

impl ServiceHttpHandlerBuilder<String> {
    /// Builds the Service::HttpHandler variant
    pub fn build(self) -> Service {
        Service::HttpHandler {
            name: self.name,
            base: self.base,
            group: self.group,
            volumes: self.volumes,
            envs: self.envs,
            depends_on: self.depends_on,
            setup: self.setup,
            port: self.port,
            cpus: self.cpus,
            ram: self.ram,
        }
    }
}

impl<Name> ServicePrecursorBuilder<Name> {
    /// Sets the base image for the service
    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }

    /// Sets the volumes for the service
    pub fn volumes(mut self, volumes: impl IntoIterator<Item = ServiceVolume>) -> Self {
        self.volumes = volumes.into_iter().collect();
        self
    }

    /// Sets the environment groups for the service
    pub fn envs(mut self, envs: impl IntoIterator<Item = String>) -> Self {
        self.envs = envs.into_iter().collect();
        self
    }

    /// Sets the dependencies for the service
    pub fn depends_on(mut self, depends_on: impl IntoIterator<Item = String>) -> Self {
        self.depends_on = depends_on.into_iter().collect();
        self
    }

    /// Sets the setup commands for the service
    pub fn setup(mut self, setup: impl IntoIterator<Item = String>) -> Self {
        self.setup = setup.into_iter().collect();
        self
    }

    /// Sets the name for the service
    pub fn name(self, name: impl Into<String>) -> ServicePrecursorBuilder<String> {
        ServicePrecursorBuilder {
            name: name.into(),
            base: self.base,
            volumes: self.volumes,
            envs: self.envs,
            depends_on: self.depends_on,
            setup: self.setup,
        }
    }
}

impl ServicePrecursorBuilder<String> {
    /// Builds the Service::Precursor variant
    pub fn build(self) -> Service {
        Service::Precursor {
            name: self.name,
            base: self.base,
            volumes: self.volumes,
            envs: self.envs,
            depends_on: self.depends_on,
            setup: self.setup,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for ServiceDefaultBuilder<()> {
    fn default() -> Self {
        Self {
            name: (),
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

impl Default for ServiceHttpHandlerBuilder<()> {
    fn default() -> Self {
        Self {
            name: (),
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
}

impl Default for ServicePrecursorBuilder<()> {
    fn default() -> Self {
        Self {
            name: (),
            base: None,
            volumes: vec![],
            envs: vec![],
            depends_on: vec![],
            setup: vec![],
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::config::PathPair;

    use super::*;

    #[test]
    fn test_service_builder_default() -> anyhow::Result<()> {
        let mut scripts = HashMap::new();
        scripts.insert("start".to_string(), "./app".to_string());

        let service = ServiceDefaultBuilder::default()
            .name("test-service")
            .base("ubuntu:24.04")
            .group("app")
            .volumes(vec![ServiceVolume::builder()
                .name("main".to_string())
                .mount(PathPair::Same("/app".parse()?))
                .build()])
            .envs(vec!["main".to_string()])
            .depends_on(vec!["db".to_string()])
            .setup(vec!["apt update".to_string()])
            .scripts(scripts)
            .port("8080:80".parse()?)
            .workdir("/app")
            .command("./app")
            .args(vec!["--port".to_string(), "80".to_string()])
            .cpus(2)
            .ram(1024)
            .build();

        match service {
            Service::Default {
                name,
                base,
                group,
                volumes,
                envs,
                depends_on,
                setup,
                scripts,
                port,
                workdir,
                command,
                args,
                cpus,
                ram,
            } => {
                assert_eq!(name, "test-service");
                assert_eq!(base, Some("ubuntu:24.04".to_string()));
                assert_eq!(group, Some("app".to_string()));
                assert_eq!(volumes.len(), 1);
                assert_eq!(envs, vec!["main"]);
                assert_eq!(depends_on, vec!["db"]);
                assert_eq!(setup, vec!["apt update"]);
                assert_eq!(scripts.get("start"), Some(&"./app".to_string()));
                assert_eq!(port, Some("8080:80".parse()?));
                assert_eq!(workdir, Some("/app".to_string()));
                assert_eq!(command, Some("./app".to_string()));
                assert_eq!(args, vec!["--port", "80"]);
                assert_eq!(cpus, 2);
                assert_eq!(ram, 1024);
            }
            _ => panic!("Expected Service::Default variant"),
        }

        Ok(())
    }

    #[test]
    fn test_service_builder_default_minimal() {
        let service = ServiceDefaultBuilder::default()
            .name("minimal-service")
            .build();

        match service {
            Service::Default {
                name,
                base,
                group,
                volumes,
                envs,
                depends_on,
                setup,
                scripts,
                port,
                workdir,
                command,
                args,
                cpus,
                ram,
            } => {
                assert_eq!(name, "minimal-service");
                assert_eq!(base, None);
                assert_eq!(group, None);
                assert!(volumes.is_empty());
                assert!(envs.is_empty());
                assert!(depends_on.is_empty());
                assert!(setup.is_empty());
                assert!(scripts.is_empty());
                assert_eq!(port, None);
                assert_eq!(workdir, None);
                assert_eq!(command, None);
                assert!(args.is_empty());
                assert_eq!(cpus, Monocore::default_num_vcpus());
                assert_eq!(ram, Monocore::default_ram_mib());
            }
            _ => panic!("Expected Service::Default variant"),
        }
    }

    #[test]
    fn test_service_builder_http_handler() -> anyhow::Result<()> {
        let service = ServiceHttpHandlerBuilder::default()
            .name("test-handler")
            .base("ubuntu:24.04")
            .group("app")
            .volumes(vec![ServiceVolume::builder()
                .name("main".to_string())
                .mount(PathPair::Same("/app".parse()?))
                .build()])
            .envs(vec!["main".to_string()])
            .depends_on(vec!["db".to_string()])
            .setup(vec!["apt update".to_string()])
            .port("8080:80".parse()?)
            .cpus(2)
            .ram(1024)
            .build();

        match service {
            Service::HttpHandler {
                name,
                base,
                group,
                volumes,
                envs,
                depends_on,
                setup,
                port,
                cpus,
                ram,
            } => {
                assert_eq!(name, "test-handler");
                assert_eq!(base, Some("ubuntu:24.04".to_string()));
                assert_eq!(group, Some("app".to_string()));
                assert_eq!(volumes.len(), 1);
                assert_eq!(envs, vec!["main"]);
                assert_eq!(depends_on, vec!["db"]);
                assert_eq!(setup, vec!["apt update"]);
                assert_eq!(port, Some("8080:80".parse()?));
                assert_eq!(cpus, 2);
                assert_eq!(ram, 1024);
            }
            _ => panic!("Expected Service::HttpHandler variant"),
        }

        Ok(())
    }

    #[test]
    fn test_service_builder_http_handler_minimal() {
        let service = ServiceHttpHandlerBuilder::default()
            .name("minimal-handler")
            .build();

        match service {
            Service::HttpHandler {
                name,
                base,
                group,
                volumes,
                envs,
                depends_on,
                setup,
                port,
                cpus,
                ram,
            } => {
                assert_eq!(name, "minimal-handler");
                assert_eq!(base, None);
                assert_eq!(group, None);
                assert!(volumes.is_empty());
                assert!(envs.is_empty());
                assert!(depends_on.is_empty());
                assert!(setup.is_empty());
                assert_eq!(port, None);
                assert_eq!(cpus, Monocore::default_num_vcpus());
                assert_eq!(ram, Monocore::default_ram_mib());
            }
            _ => panic!("Expected Service::HttpHandler variant"),
        }
    }

    #[test]
    fn test_service_builder_precursor() -> anyhow::Result<()> {
        let service = ServicePrecursorBuilder::default()
            .name("test-precursor")
            .base("ubuntu:24.04")
            .volumes(vec![ServiceVolume::builder()
                .name("main".to_string())
                .mount(PathPair::Same("/app".parse()?))
                .build()])
            .envs(vec!["main".to_string()])
            .depends_on(vec!["db".to_string()])
            .setup(vec!["apt update".to_string()])
            .build();

        match service {
            Service::Precursor {
                name,
                base,
                volumes,
                envs,
                depends_on,
                setup,
            } => {
                assert_eq!(name, "test-precursor");
                assert_eq!(base, Some("ubuntu:24.04".to_string()));
                assert_eq!(volumes.len(), 1);
                assert_eq!(envs, vec!["main"]);
                assert_eq!(depends_on, vec!["db"]);
                assert_eq!(setup, vec!["apt update"]);
            }
            _ => panic!("Expected Service::Precursor variant"),
        }

        Ok(())
    }

    #[test]
    fn test_service_builder_precursor_minimal() {
        let service = ServicePrecursorBuilder::default()
            .name("minimal-precursor")
            .build();

        match service {
            Service::Precursor {
                name,
                base,
                volumes,
                envs,
                depends_on,
                setup,
            } => {
                assert_eq!(name, "minimal-precursor");
                assert_eq!(base, None);
                assert!(volumes.is_empty());
                assert!(envs.is_empty());
                assert!(depends_on.is_empty());
                assert!(setup.is_empty());
            }
            _ => panic!("Expected Service::Precursor variant"),
        }
    }
}

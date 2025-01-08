use crate::config::{
    monocore::{Monocore, Service},
    PortPair,
};

use super::{EnvPair, PathPair, VolumeMount};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A builder for creating a `Service`
pub struct ServiceBuilder<Name> {
    name: Name,
    base: Option<String>,
    group: Option<String>,
    volumes: Vec<PathPair>,
    envs: Vec<EnvPair>,
    group_volumes: Vec<VolumeMount>,
    group_envs: Vec<String>,
    depends_on: Vec<String>,
    ports: Vec<PortPair>,
    workdir: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    cpus: u8,
    ram: u32,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<Name> ServiceBuilder<Name> {
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
    pub fn volumes(mut self, volumes: impl IntoIterator<Item = PathPair>) -> Self {
        self.volumes = volumes.into_iter().collect();
        self
    }

    /// Sets the environment variables for the service
    pub fn envs(mut self, envs: impl IntoIterator<Item = EnvPair>) -> Self {
        self.envs = envs.into_iter().collect();
        self
    }

    /// Sets the group  volumes for the service
    pub fn group_volumes(mut self, volumes: impl IntoIterator<Item = VolumeMount>) -> Self {
        self.group_volumes = volumes.into_iter().collect();
        self
    }

    /// Sets the group environment variables for the service
    pub fn group_envs(mut self, envs: impl IntoIterator<Item = String>) -> Self {
        self.group_envs = envs.into_iter().collect();
        self
    }

    /// Sets the dependencies for the service
    pub fn depends_on(mut self, depends_on: impl IntoIterator<Item = String>) -> Self {
        self.depends_on = depends_on.into_iter().collect();
        self
    }

    /// Sets the port mappings for the service
    pub fn ports(mut self, ports: impl IntoIterator<Item = PortPair>) -> Self {
        self.ports = ports.into_iter().collect();
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
    pub fn args<'a>(mut self, args: impl IntoIterator<Item = &'a str>) -> Self {
        self.args = args.into_iter().map(|s| s.to_string()).collect();
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
    pub fn name(self, name: impl Into<String>) -> ServiceBuilder<String> {
        ServiceBuilder {
            name: name.into(),
            base: self.base,
            group: self.group,
            volumes: self.volumes,
            envs: self.envs,
            group_volumes: self.group_volumes,
            group_envs: self.group_envs,
            depends_on: self.depends_on,
            ports: self.ports,
            workdir: self.workdir,
            command: self.command,
            args: self.args,
            cpus: self.cpus,
            ram: self.ram,
        }
    }
}

impl ServiceBuilder<String> {
    /// Builds the Service::Default variant
    pub fn build(self) -> Service {
        Service {
            name: self.name,
            base: self.base,
            group: self.group,
            volumes: self.volumes,
            envs: self.envs,
            group_volumes: self.group_volumes,
            group_envs: self.group_envs,
            depends_on: self.depends_on,
            ports: self.ports,
            workdir: self.workdir,
            command: self.command,
            args: self.args,
            cpus: self.cpus,
            ram: self.ram,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for ServiceBuilder<()> {
    fn default() -> Self {
        Self {
            name: (),
            base: None,
            group: None,
            volumes: vec![],
            envs: vec![],
            group_volumes: vec![],
            group_envs: vec![],
            depends_on: vec![],
            ports: vec![],
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
    use crate::config::PathPair;

    use super::*;

    #[test]
    fn test_service_builder_default() -> anyhow::Result<()> {
        let service = ServiceBuilder::default()
            .name("test-service")
            .base("ubuntu:24.04")
            .group("app")
            .volumes(["/app;".parse()?])
            .envs(["ENV=main".parse()?])
            .group_volumes([VolumeMount::builder()
                .name("main".to_string())
                .mount(PathPair::Same("/app".parse()?))
                .build()])
            .group_envs(["main".to_string()])
            .depends_on(["db".to_string()])
            .ports(["8080:80".parse()?])
            .workdir("/app")
            .command("./app")
            .args(["--port", "80"])
            .cpus(2)
            .ram(1024)
            .build();

        assert_eq!(service.name, "test-service");
        assert_eq!(service.base, Some("ubuntu:24.04".to_string()));
        assert_eq!(service.group, Some("app".to_string()));
        assert_eq!(service.volumes.len(), 1);
        assert_eq!(service.envs, vec!["ENV=main".parse()?]);
        assert_eq!(service.group_volumes.len(), 1);
        assert_eq!(service.group_envs, vec!["main".to_string()]);
        assert_eq!(service.depends_on, vec!["db".to_string()]);
        assert_eq!(service.ports, vec!["8080:80".parse()?]);
        assert_eq!(service.workdir, Some("/app".to_string()));
        assert_eq!(service.command, Some("./app".to_string()));
        assert_eq!(service.args, vec!["--port", "80"]);
        assert_eq!(service.cpus, 2);
        assert_eq!(service.ram, 1024);

        Ok(())
    }

    #[test]
    fn test_service_builder_default_minimal() {
        let service = ServiceBuilder::default().name("minimal-service").build();

        assert_eq!(service.name, "minimal-service");
        assert_eq!(service.base, None);
        assert_eq!(service.group, None);
        assert!(service.volumes.is_empty());
        assert!(service.envs.is_empty());
        assert!(service.group_volumes.is_empty());
        assert!(service.group_envs.is_empty());
        assert!(service.depends_on.is_empty());
        assert!(service.ports.is_empty());
        assert_eq!(service.workdir, None);
        assert_eq!(service.command, None);
        assert!(service.args.is_empty());
        assert_eq!(service.cpus, Monocore::default_num_vcpus());
        assert_eq!(service.ram, Monocore::default_ram_mib());
    }
}

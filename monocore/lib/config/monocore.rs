//! Monocore configuration types and helpers.

use std::{collections::HashMap, net::Ipv4Addr};

use getset::Getters;
use ipnetwork::Ipv4Network as Ipv4Net;
use oci_spec::distribution::Reference;
use semver::Version;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use typed_path::Utf8UnixPathBuf;

use super::{EnvPair, PathPair, PortPair, DEFAULT_NUM_VCPUS, DEFAULT_RAM_MIB};

use crate::MonocoreResult;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The monocore configuration.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Monocore {
    /// The metadata about the configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) meta: Option<Meta>,

    /// The files to import.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) requires: Option<Vec<Require>>,

    /// The builds to run.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) builds: Option<Vec<Build>>,

    /// The sandboxes to run.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) sandboxes: Option<Vec<Sandbox>>,

    /// The groups to run the sandboxes in.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(super) groups: Option<Vec<Group>>,
}

/// The metadata about the configuration.
#[derive(Debug, Default, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Meta {
    /// The authors of the configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) authors: Option<Vec<String>>,
}

/// Import name mapping.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Require {
    /// The path to the file to import.
    #[builder(setter(transform = |path: impl AsRef<str>| Utf8UnixPathBuf::from(path.as_ref().to_string())))]
    #[serde(
        serialize_with = "serialize_path",
        deserialize_with = "deserialize_path"
    )]
    pub(super) path: Utf8UnixPathBuf,

    /// The name mappings for imported items, mapping from original name to new name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) items: Option<HashMap<String, String>>,
}

/// A build to run.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Build {
    /// The name of the build.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The image to use.
    pub(super) image: Reference,

    /// The amount of RAM in MiB to use.
    #[serde(default = "Monocore::default_ram_mib")]
    #[builder(default = Monocore::default_ram_mib())]
    pub(super) ram: u32,

    /// The number of vCPUs to use.
    #[serde(default = "Monocore::default_num_vcpus")]
    #[builder(default = Monocore::default_num_vcpus())]
    pub(super) cpus: u8,

    /// The volumes to mount.
    #[builder(default)]
    pub(super) volumes: Vec<PathPair>,

    /// The ports to expose.
    #[serde(default)]
    #[builder(default)]
    pub(super) ports: Vec<PortPair>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) envs: Option<Vec<EnvPair>>,

    /// The groups to run in.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) groups: Option<Vec<String>>,

    /// The builds to depend on.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) depends_on: Option<Vec<String>>,

    /// The working directory to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) workdir: Option<Utf8UnixPathBuf>,

    /// The shell to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) shell: Option<String>,

    /// The steps to run.
    #[builder(default)]
    pub(super) steps: Vec<String>,

    /// The files to import.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(
        serialize_with = "serialize_optional_path_map",
        deserialize_with = "deserialize_optional_path_map"
    )]
    #[builder(default)]
    pub(super) imports: Option<HashMap<String, Utf8UnixPathBuf>>,

    /// The artifacts produced by the build.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(
        serialize_with = "serialize_optional_path_map",
        deserialize_with = "deserialize_optional_path_map"
    )]
    #[builder(default)]
    pub(super) exports: Option<HashMap<String, Utf8UnixPathBuf>>,
}

/// Network reach configuration for a group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroupNetworkReach {
    /// Sandboxes can only communicate within their subnet
    #[serde(rename = "local")]
    Local,

    /// Sandboxes can communicate with other groups on 172.16.0.0/12 range or any other non-private address
    #[serde(rename = "public")]
    Public,

    /// Sandboxes can communicate with any address
    #[serde(rename = "any")]
    Any,

    /// Sandboxes cannot communicate with any other sandboxes
    #[serde(rename = "none")]
    None,
}

/// Network reach configuration for a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SandboxNetworkReach {
    /// Sandbox can communicate with other groups on 172.16.0.0/12 range or any other non-private address
    #[serde(rename = "public")]
    Public,

    /// Sandbox can communicate with any address
    #[serde(rename = "any")]
    Any,

    /// Sandbox cannot communicate with any other sandboxes
    #[serde(rename = "none")]
    None,
}

/// Network configuration for a group.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct GroupNetworkConfig {
    /// The subnet CIDR for the group. Must be an IPv4 network.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) subnet: Option<Ipv4Net>,

    /// The network reach configuration.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "GroupNetworkConfig::default_reach"
    )]
    #[builder(default = GroupNetworkConfig::default_reach())]
    pub(super) reach: Option<GroupNetworkReach>,
}

/// Network configuration for a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct SandboxNetworkConfig {
    /// The network reach configuration.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "SandboxNetworkConfig::default_reach"
    )]
    #[builder(default = SandboxNetworkConfig::default_reach())]
    pub(super) reach: Option<SandboxNetworkReach>,

    /// The domain name to IP address mappings.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) domains: Option<HashMap<String, Vec<Ipv4Addr>>>,
}

/// The sandbox to run.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Sandbox {
    /// The name of the sandbox.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The version of the sandbox.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) version: Option<Version>,

    /// The image to use.
    pub(super) image: Reference,

    /// The amount of RAM in MiB to use.
    #[serde(default = "Monocore::default_ram_mib")]
    #[builder(default = Monocore::default_ram_mib())]
    pub(super) ram: u32,

    /// The number of vCPUs to use.
    #[serde(default = "Monocore::default_num_vcpus")]
    #[builder(default = Monocore::default_num_vcpus())]
    pub(super) cpus: u8,

    /// The volumes to mount.
    #[builder(default)]
    pub(super) volumes: Vec<PathPair>,

    /// The ports to expose.
    #[serde(default)]
    #[builder(default)]
    pub(super) ports: Vec<PortPair>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) envs: Option<Vec<EnvPair>>,

    /// The groups to run in.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) groups: Option<Vec<String>>,

    /// The sandboxes to depend on.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) depends_on: Option<Vec<String>>,

    /// The working directory to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) workdir: Option<Utf8UnixPathBuf>,

    /// The shell to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) shell: Option<String>,

    /// The scripts that can be run.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) scripts: Option<HashMap<String, Vec<String>>>,

    /// The files to import.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) imports: Option<HashMap<String, String>>,

    /// The artifacts produced by the sandbox.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) exports: Option<HashMap<String, String>>,

    /// The network configuration for the sandbox.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) network: Option<SandboxNetworkConfig>,

    /// The group volumes to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) group_volumes: Option<HashMap<String, PathPair>>,

    /// The group environment variables to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) group_envs: Option<Vec<String>>,
}

/// The group to run the sandboxes in.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Group {
    /// The name of the group.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The network configuration for the group.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) network: Option<GroupNetworkConfig>,

    /// The volumes to mount.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(
        serialize_with = "serialize_optional_path_map",
        deserialize_with = "deserialize_optional_path_map"
    )]
    #[builder(default)]
    pub(super) volumes: Option<HashMap<String, Utf8UnixPathBuf>>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) envs: Option<HashMap<String, Vec<EnvPair>>>,
}

//--------------------------------------------------------------------------------------------------
// Types: Builders
//--------------------------------------------------------------------------------------------------

/// Builder for Monocore configuration.
#[derive(Default)]
pub struct MonocoreBuilder {
    meta: Option<Meta>,
    requires: Option<Vec<Require>>,
    builds: Option<Vec<Build>>,
    sandboxes: Option<Vec<Sandbox>>,
    groups: Option<Vec<Group>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Monocore {
    /// The maximum sandbox dependency chain length.
    pub const MAX_DEPENDENCY_DEPTH: usize = 32;

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
        self.groups
            .as_ref()
            .and_then(|groups| groups.iter().find(|g| g.get_name() == group_name))
    }

    /// Get a sandbox by name in this configuration
    pub fn get_sandbox(&self, sandbox_name: &str) -> Option<&Sandbox> {
        self.sandboxes
            .as_ref()
            .and_then(|sandboxes| sandboxes.iter().find(|s| s.get_name() == sandbox_name))
    }

    /// Validates the configuration.
    pub fn validate(&self) -> MonocoreResult<()> {
        // TODO: Add validation logic here
        Ok(())
    }
}

impl SandboxNetworkConfig {
    /// Returns the default network reach configuration.
    pub fn default_reach() -> Option<SandboxNetworkReach> {
        Some(SandboxNetworkReach::Public)
    }
}

impl GroupNetworkConfig {
    /// Returns the default network reach configuration.
    pub fn default_reach() -> Option<GroupNetworkReach> {
        Some(GroupNetworkReach::Local)
    }
}

impl MonocoreBuilder {
    /// Creates a new MonocoreBuilder instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the metadata for the configuration
    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Sets the files to import
    pub fn requires(mut self, requires: Vec<Require>) -> Self {
        self.requires = Some(requires);
        self
    }

    /// Sets the builds to run
    pub fn builds(mut self, builds: Vec<Build>) -> Self {
        self.builds = Some(builds);
        self
    }

    /// Sets the sandboxes to run
    pub fn sandboxes(mut self, sandboxes: Vec<Sandbox>) -> Self {
        self.sandboxes = Some(sandboxes);
        self
    }

    /// Sets the groups to run the sandboxes in
    pub fn groups(mut self, groups: Vec<Group>) -> Self {
        self.groups = Some(groups);
        self
    }

    /// Builds the Monocore configuration with validation
    pub fn build(self) -> MonocoreResult<Monocore> {
        let monocore = self.build_unchecked();
        monocore.validate()?;
        Ok(monocore)
    }

    /// Builds the Monocore configuration without validation
    pub fn build_unchecked(self) -> Monocore {
        Monocore {
            meta: self.meta,
            requires: self.requires,
            builds: self.builds,
            sandboxes: self.sandboxes,
            groups: self.groups,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Serialization helpers
//--------------------------------------------------------------------------------------------------

fn serialize_path<S>(path: &Utf8UnixPathBuf, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(path.as_str())
}

fn deserialize_path<'de, D>(deserializer: D) -> Result<Utf8UnixPathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(Utf8UnixPathBuf::from(s))
}

fn serialize_optional_path<S>(
    path: &Option<Utf8UnixPathBuf>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match path {
        Some(p) => serializer.serialize_str(p.as_str()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_optional_path<'de, D>(deserializer: D) -> Result<Option<Utf8UnixPathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|s| Ok(Utf8UnixPathBuf::from(s)))
        .transpose()
}

fn serialize_optional_path_map<S>(
    map: &Option<HashMap<String, Utf8UnixPathBuf>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match map {
        Some(m) => {
            use serde::ser::SerializeMap;
            let mut map_ser = serializer.serialize_map(Some(m.len()))?;
            for (k, v) in m {
                map_ser.serialize_entry(k, v.as_str())?;
            }
            map_ser.end()
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_optional_path_map<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, Utf8UnixPathBuf>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<HashMap<String, String>>::deserialize(deserializer).map(|opt_map| {
        opt_map.map(|string_map| {
            string_map
                .into_iter()
                .map(|(k, v)| (k, Utf8UnixPathBuf::from(v)))
                .collect()
        })
    })
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    #[test]
    fn test_basic_config_serialization() {
        let config = Monocore {
            meta: Some(Meta {
                authors: Some(vec!["Test Author".to_string()]),
            }),
            requires: None,
            builds: None,
            sandboxes: None,
            groups: None,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: Monocore = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_full_config() {
        let yaml = r#"
meta:
  authors:
    - "Test Author"
requires:
  - path: "/path/to/file"
    items:
      original_name: new_name
builds:
  - name: "build0"
    image: "ubuntu:20.04"
    ram: 1024
    cpus: 1
    steps:
      - "echo 'Building dependencies'"
  - name: "build1"
    image: "ubuntu:latest"
    ram: 2048
    cpus: 2
    volumes:
      - "/host/path:/container/path"
    ports:
      - host: 8080
        container: 80
    envs:
      - "ENV_VAR=value"
    groups:
      - "group1"
    depends_on:
      - "build0"
    workdir: "/app"
    shell: "/bin/bash"
    steps:
      - "echo 'Hello World'"
    imports:
      file1: "/path/to/import"
    exports:
      file2: "/path/to/export"
sandboxes:
  - name: "sandbox0"
    image: "alpine:3.14"
    ram: 512
    cpus: 1
    groups:
      - "group1"
  - name: "sandbox1"
    version: "1.0.0"
    image: "alpine:latest"
    ram: 1024
    cpus: 1
    volumes:
      - "/host/path:/container/path"
    ports:
      - host: 8080
        container: 80
    envs:
      - "SANDBOX_ENV=sandbox_value"
    groups:
      - "group1"
    depends_on:
      - "sandbox0"
    workdir: "/sandbox/app"
    shell: "/bin/sh"
    scripts:
      start:
        - "echo 'Starting sandbox'"
        - "./run.sh"
      stop:
        - "echo 'Stopping sandbox'"
    imports:
      config: "/path/to/config"
    exports:
      logs: "/path/to/logs"
    network:
      reach: public
      domains:
        "example.com":
          - "10.0.0.1"
    group_volumes:
      shared_data: "/shared/host/path:/shared/container/path"
    group_envs:
      - "GROUP_VAR"
groups:
  - name: "group1"
    network:
      reach: local
      subnet: "10.0.0.0/24"
    volumes:
      shared: "/shared/path"
    envs:
      GROUP_VAR:
        - "KEY1=value1"
        - "KEY2=value2"
"#;

        let config: Monocore = serde_yaml::from_str(yaml).unwrap();

        // Verify meta
        assert_eq!(
            config
                .get_meta()
                .as_ref()
                .unwrap()
                .get_authors()
                .as_ref()
                .unwrap()[0],
            "Test Author"
        );

        // Verify requires
        let requires = config.get_requires().as_ref().unwrap();
        assert_eq!(requires[0].get_path().as_str(), "/path/to/file");
        assert_eq!(
            requires[0]
                .get_items()
                .as_ref()
                .unwrap()
                .get("original_name")
                .unwrap(),
            "new_name"
        );

        // Verify builds
        let builds = config.get_builds().as_ref().unwrap();

        // Verify build0
        let build0 = &builds[0];
        assert_eq!(build0.get_name(), "build0");
        assert_eq!(build0.get_image().to_string(), "ubuntu:20.04");
        assert_eq!(build0.get_ram(), &1024);
        assert_eq!(build0.get_cpus(), &1);
        assert_eq!(build0.get_steps()[0], "echo 'Building dependencies'");

        // Verify build1
        let build1 = &builds[1];
        assert_eq!(build1.get_name(), "build1");
        assert_eq!(build1.get_image().to_string(), "ubuntu:latest");
        assert_eq!(build1.get_ram(), &2048);
        assert_eq!(build1.get_cpus(), &2);
        assert_eq!(build1.get_volumes()[0].get_host().as_str(), "/host/path");
        assert_eq!(
            build1.get_volumes()[0].get_guest().as_str(),
            "/container/path"
        );
        assert_eq!(build1.get_ports()[0].get_host(), 8080);
        assert_eq!(build1.get_ports()[0].get_guest(), 80);
        assert_eq!(build1.get_envs().as_ref().unwrap()[0].get_name(), "ENV_VAR");
        assert_eq!(build1.get_envs().as_ref().unwrap()[0].get_value(), "value");
        assert_eq!(build1.get_groups().as_ref().unwrap()[0], "group1");
        assert_eq!(build1.get_depends_on().as_ref().unwrap()[0], "build0");
        assert_eq!(build1.get_workdir().as_ref().unwrap().as_str(), "/app");
        assert_eq!(build1.get_shell().as_ref().unwrap(), "/bin/bash");
        assert_eq!(build1.get_steps()[0], "echo 'Hello World'");
        assert_eq!(
            build1.get_imports().as_ref().unwrap().get("file1").unwrap(),
            "/path/to/import"
        );
        assert_eq!(
            build1.get_exports().as_ref().unwrap().get("file2").unwrap(),
            "/path/to/export"
        );

        // Verify sandboxes
        let sandboxes = config.get_sandboxes().as_ref().unwrap();

        // Verify sandbox0
        let sandbox0 = &sandboxes[0];
        assert_eq!(sandbox0.get_name(), "sandbox0");
        assert_eq!(sandbox0.get_image().to_string(), "alpine:3.14");
        assert_eq!(sandbox0.get_ram(), &512);
        assert_eq!(sandbox0.get_cpus(), &1);
        assert_eq!(sandbox0.get_groups().as_ref().unwrap()[0], "group1");

        // Verify sandbox1
        let sandbox1 = &sandboxes[1];
        assert_eq!(sandbox1.get_name(), "sandbox1");
        assert_eq!(
            sandbox1.get_version().as_ref().unwrap().to_string(),
            "1.0.0"
        );
        assert_eq!(sandbox1.get_image().to_string(), "alpine:latest");
        assert_eq!(sandbox1.get_ram(), &1024);
        assert_eq!(sandbox1.get_cpus(), &1);
        assert_eq!(sandbox1.get_volumes()[0].get_host().as_str(), "/host/path");
        assert_eq!(
            sandbox1.get_volumes()[0].get_guest().as_str(),
            "/container/path"
        );
        assert_eq!(sandbox1.get_ports()[0].get_host(), 8080);
        assert_eq!(sandbox1.get_ports()[0].get_guest(), 80);
        assert_eq!(
            sandbox1.get_envs().as_ref().unwrap()[0].get_name(),
            "SANDBOX_ENV"
        );
        assert_eq!(
            sandbox1.get_envs().as_ref().unwrap()[0].get_value(),
            "sandbox_value"
        );
        assert_eq!(sandbox1.get_groups().as_ref().unwrap()[0], "group1");
        assert_eq!(sandbox1.get_depends_on().as_ref().unwrap()[0], "sandbox0");
        assert_eq!(
            sandbox1.get_workdir().as_ref().unwrap().as_str(),
            "/sandbox/app"
        );
        assert_eq!(sandbox1.get_shell().as_ref().unwrap(), "/bin/sh");

        // Verify sandbox1 scripts
        let scripts = sandbox1.get_scripts().as_ref().unwrap();
        let start_script = scripts.get("start").unwrap();
        assert_eq!(start_script[0], "echo 'Starting sandbox'");
        assert_eq!(start_script[1], "./run.sh");
        let stop_script = scripts.get("stop").unwrap();
        assert_eq!(stop_script[0], "echo 'Stopping sandbox'");

        // Verify sandbox1 imports/exports
        assert_eq!(
            sandbox1
                .get_imports()
                .as_ref()
                .unwrap()
                .get("config")
                .unwrap(),
            "/path/to/config"
        );
        assert_eq!(
            sandbox1
                .get_exports()
                .as_ref()
                .unwrap()
                .get("logs")
                .unwrap(),
            "/path/to/logs"
        );

        // Verify sandbox1 network
        let network = sandbox1.get_network().as_ref().unwrap();
        assert!(matches!(
            network.get_reach().as_ref().unwrap(),
            SandboxNetworkReach::Public
        ));
        assert_eq!(
            network
                .get_domains()
                .as_ref()
                .unwrap()
                .get("example.com")
                .unwrap()[0]
                .to_string(),
            "10.0.0.1"
        );

        // Verify sandbox1 group volumes and envs
        let group_volumes = sandbox1.get_group_volumes().as_ref().unwrap();
        assert_eq!(
            group_volumes
                .get("shared_data")
                .unwrap()
                .get_host()
                .as_str(),
            "/shared/host/path"
        );
        assert_eq!(
            group_volumes
                .get("shared_data")
                .unwrap()
                .get_guest()
                .as_str(),
            "/shared/container/path"
        );
        let group_envs = sandbox1.get_group_envs().as_ref().unwrap();
        assert_eq!(group_envs[0], "GROUP_VAR");

        // Verify groups
        let groups = config.get_groups().as_ref().unwrap();
        let group = &groups[0];
        assert_eq!(group.get_name(), "group1");

        // Verify group network
        let group_network = group.get_network().as_ref().unwrap();
        assert!(matches!(
            group_network.get_reach().as_ref().unwrap(),
            GroupNetworkReach::Local
        ));
        assert_eq!(
            group_network.get_subnet().unwrap().to_string(),
            "10.0.0.0/24"
        );

        // Verify group volumes and envs
        assert_eq!(
            group
                .get_volumes()
                .as_ref()
                .unwrap()
                .get("shared")
                .unwrap()
                .as_str(),
            "/shared/path"
        );
        let group_vars = group.get_envs().as_ref().unwrap().get("GROUP_VAR").unwrap();
        assert_eq!(group_vars[0].get_name(), "KEY1");
        assert_eq!(group_vars[0].get_value(), "value1");
        assert_eq!(group_vars[1].get_name(), "KEY2");
        assert_eq!(group_vars[1].get_value(), "value2");

        // Test serialization roundtrip
        let serialized = serde_yaml::to_string(&config).unwrap();
        let deserialized: Monocore = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_network_config() {
        let yaml = r#"
sandboxes:
  - name: "sandbox1"
    image: "alpine:latest"
    network:
      reach: any
      domains:
        "example.com":
          - "10.0.0.1"
groups:
  - name: "group1"
    network:
      reach: local
      subnet: "10.0.0.0/24"
"#;

        let config: Monocore = serde_yaml::from_str(yaml).unwrap();

        // Verify sandbox network config
        let sandbox = &config.get_sandboxes().as_ref().unwrap()[0];
        let network = sandbox.get_network().as_ref().unwrap();
        assert!(matches!(
            network.get_reach().as_ref().unwrap(),
            SandboxNetworkReach::Any
        ));
        assert_eq!(
            network
                .get_domains()
                .as_ref()
                .unwrap()
                .get("example.com")
                .unwrap()[0]
                .to_string(),
            "10.0.0.1"
        );

        // Verify group network config
        let group = &config.get_groups().as_ref().unwrap()[0];
        let network = group.get_network().as_ref().unwrap();
        assert!(matches!(
            network.get_reach().as_ref().unwrap(),
            GroupNetworkReach::Local
        ));
        assert_eq!(network.get_subnet().unwrap().to_string(), "10.0.0.0/24");
    }

    #[test]
    fn test_builder_pattern() {
        let config = MonocoreBuilder::new()
            .meta(
                Meta::builder()
                    .authors(Some(vec!["Test Author".to_string()]))
                    .build(),
            )
            .sandboxes(vec![Sandbox::builder()
                .name("sandbox1")
                .image("alpine:latest".parse().unwrap())
                .build()])
            .build()
            .unwrap();

        assert_eq!(
            config
                .get_meta()
                .as_ref()
                .unwrap()
                .get_authors()
                .as_ref()
                .unwrap()[0],
            "Test Author"
        );
        assert_eq!(
            config.get_sandboxes().as_ref().unwrap()[0].get_name(),
            "sandbox1"
        );
    }
}

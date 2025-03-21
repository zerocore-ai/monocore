//! Monocore configuration types and helpers.

use std::{borrow::Cow, collections::HashMap, net::Ipv4Addr};

use getset::Getters;
use ipnetwork::Ipv4Network as Ipv4Net;
use semver::Version;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{
        EnvPair, PathPair, PortPair, ReferencePath, DEFAULT_NUM_VCPUS, DEFAULT_RAM_MIB,
        DEFAULT_SHELL,
    },
    MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The monocore configuration.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct MonocoreConfig {
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
#[derive(Debug, Default, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Meta {
    /// The authors of the configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) authors: Option<Vec<String>>,

    /// The description of the sandbox.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) description: Option<String>,

    /// The homepage of the configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) homepage: Option<String>,

    /// The repository of the configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) repository: Option<String>,

    /// The path to the readme file.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) readme: Option<Utf8UnixPathBuf>,

    /// The tags for the configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) tags: Option<Vec<String>>,

    /// The icon for the configuration.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) icon: Option<Utf8UnixPathBuf>,
}

/// Component mapping for imports.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct ComponentMapping {
    /// The alias for the component.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) as_: Option<String>,
}

/// Import configuration.
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

    /// The component mappings.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) imports: HashMap<String, ComponentMapping>,
}

/// A build to run.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Build {
    /// The name of the build.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The image to use. This can be a path to a local rootfs or an OCI image reference.
    pub(super) image: ReferencePath,

    /// The amount of RAM in MiB to use.
    #[serde(default = "MonocoreConfig::default_ram_mib")]
    #[builder(default = MonocoreConfig::default_ram_mib())]
    pub(super) ram: u32,

    /// The number of vCPUs to use.
    #[serde(default = "MonocoreConfig::default_num_vcpus")]
    #[builder(default = MonocoreConfig::default_num_vcpus())]
    pub(super) cpus: u8,

    /// The volumes to mount.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) volumes: Vec<PathPair>,

    /// The ports to expose.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) ports: Vec<PortPair>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) envs: Vec<EnvPair>,

    /// The groups to run in.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) groups: HashMap<String, GroupConfig>,

    /// The builds to depend on.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) depends_on: Vec<String>,

    /// The working directory to use.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) workdir: Option<Utf8UnixPathBuf>,

    /// The shell to use.
    #[serde(default = "Build::default_shell")]
    #[builder(default = Build::default_shell())]
    pub(super) shell: String,

    /// The scripts that can be run.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) scripts: HashMap<String, String>,

    /// The files to import.
    #[serde(
        skip_serializing_if = "HashMap::is_empty",
        default,
        serialize_with = "serialize_path_map",
        deserialize_with = "deserialize_path_map"
    )]
    #[builder(default)]
    pub(super) imports: HashMap<String, Utf8UnixPathBuf>,

    /// The artifacts produced by the build.
    #[serde(
        skip_serializing_if = "HashMap::is_empty",
        default,
        serialize_with = "serialize_path_map",
        deserialize_with = "deserialize_path_map"
    )]
    #[builder(default)]
    pub(super) exports: HashMap<String, Utf8UnixPathBuf>,
}

/// Network reach configuration for a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SandboxNetworkReach {
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
}

/// Network configuration for a sandbox in a group.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct SandboxGroupNetworkConfig {
    /// The IP address for the sandbox in this group
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) ip: Option<Ipv4Addr>,

    /// The domain names for this sandbox in the group
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) domains: Vec<String>,
}

/// Network configuration for a group.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct GroupNetworkConfig {
    /// The subnet CIDR for the group. Must be an IPv4 network.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) subnet: Option<Ipv4Net>,
}

/// Proxy configuration for a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ProxyConfig {
    /// Legacy HTTP proxy configuration.
    #[serde(rename = "legacy")]
    Legacy {
        /// The prefix to use for routing.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        prefix: Option<String>,

        /// The keep alive duration.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        keep_alive: Option<String>,

        /// The maximum number of concurrent connections.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        concurrency: Option<u32>,

        /// The port to expose.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        port: Option<PortPair>,
    },
    /// Handler-based proxy configuration.
    #[serde(rename = "handler")]
    Handler {
        /// The programming language to use.
        language: String,

        /// The prefix to use for routing.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        prefix: Option<String>,

        /// The keep alive duration.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        keep_alive: Option<String>,

        /// The maximum number of concurrent connections.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        concurrency: Option<u32>,

        /// The port to expose.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        port: Option<PortPair>,
    },
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

    /// The metadata about the sandbox.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) meta: Option<Meta>,

    /// The image to use. This can be a path to a local rootfs or an OCI image reference.
    pub(super) image: ReferencePath,

    /// The amount of RAM in MiB to use.
    #[serde(default = "MonocoreConfig::default_ram_mib")]
    #[builder(default = MonocoreConfig::default_ram_mib())]
    pub(super) ram: u32,

    /// The number of vCPUs to use.
    #[serde(default = "MonocoreConfig::default_num_vcpus")]
    #[builder(default = MonocoreConfig::default_num_vcpus())]
    pub(super) cpus: u8,

    /// The volumes to mount.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) volumes: Vec<PathPair>,

    /// The ports to expose.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) ports: Vec<PortPair>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) envs: Vec<EnvPair>,

    /// The environment file to use.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) env_file: Option<Utf8UnixPathBuf>,

    /// The groups to run in.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) groups: HashMap<String, GroupConfig>,

    /// The sandboxes to depend on.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[builder(default)]
    pub(super) depends_on: Vec<String>,

    /// The working directory to use.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "serialize_optional_path",
        deserialize_with = "deserialize_optional_path"
    )]
    #[builder(default)]
    pub(super) workdir: Option<Utf8UnixPathBuf>,

    /// The shell to use.
    #[serde(default = "Sandbox::default_shell")]
    #[builder(default = Sandbox::default_shell())]
    pub(super) shell: String,

    /// The scripts that can be run.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) scripts: HashMap<String, String>,

    /// The files to import.
    #[serde(
        skip_serializing_if = "HashMap::is_empty",
        default,
        serialize_with = "serialize_path_map",
        deserialize_with = "deserialize_path_map"
    )]
    #[builder(default)]
    pub(super) imports: HashMap<String, Utf8UnixPathBuf>,

    /// The artifacts produced by the sandbox.
    #[serde(
        skip_serializing_if = "HashMap::is_empty",
        default,
        serialize_with = "serialize_path_map",
        deserialize_with = "deserialize_path_map"
    )]
    #[builder(default)]
    pub(super) exports: HashMap<String, Utf8UnixPathBuf>,

    /// The network configuration for the sandbox.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) network: Option<SandboxNetworkConfig>,

    /// The proxy configuration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) proxy: Option<ProxyConfig>,
}

/// Configuration for a sandbox's group membership.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct GroupConfig {
    /// The volumes to mount.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) volumes: HashMap<String, PathPair>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) envs: HashMap<String, Vec<EnvPair>>,

    /// The network configuration for this sandbox in the group.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) network: Option<SandboxGroupNetworkConfig>,
}

/// The group to run the sandboxes in.
#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct Group {
    /// The name of the group.
    #[builder(setter(transform = |name: impl AsRef<str>| name.as_ref().to_string()))]
    pub(super) name: String,

    /// The version of the group.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) version: Option<Version>,

    /// The metadata about the group.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) meta: Option<Meta>,

    /// The network configuration for the group.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[builder(default)]
    pub(super) network: Option<GroupNetworkConfig>,

    /// The volumes to mount.
    #[serde(
        skip_serializing_if = "HashMap::is_empty",
        default,
        serialize_with = "serialize_path_map",
        deserialize_with = "deserialize_path_map"
    )]
    #[builder(default)]
    pub(super) volumes: HashMap<String, Utf8UnixPathBuf>,

    /// The environment variables to use.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    #[builder(default)]
    pub(super) envs: HashMap<String, Vec<EnvPair>>,
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

impl MonocoreConfig {
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

    /// Get a sandbox by name in this configuration
    pub fn get_sandbox(&self, sandbox_name: &str) -> Option<&Sandbox> {
        self.sandboxes
            .as_ref()
            .and_then(|sandboxes| sandboxes.iter().find(|s| s.get_name() == sandbox_name))
    }

    /// Get a group by name in this configuration
    pub fn get_group(&self, group_name: &str) -> Option<&Group> {
        self.groups
            .as_ref()
            .and_then(|groups| groups.iter().find(|g| g.get_name() == group_name))
    }

    /// Get a build by name in this configuration
    pub fn get_build(&self, build_name: &str) -> Option<&Build> {
        self.builds
            .as_ref()
            .and_then(|builds| builds.iter().find(|b| b.get_name() == build_name))
    }

    /// Validates the configuration.
    pub fn validate(&self) -> MonocoreResult<()> {
        // TODO: Add validation logic here
        Ok(())
    }
}

impl Build {
    /// Returns the default shell.
    pub fn default_shell() -> String {
        DEFAULT_SHELL.to_string()
    }
}

impl Sandbox {
    /// Returns the default shell.
    pub fn default_shell() -> String {
        DEFAULT_SHELL.to_string()
    }

    /// Returns the start script for the sandbox.
    pub fn get_start_script(&self) -> &str {
        if let Some(script) = self.scripts.get("start") {
            script
        } else {
            &self.shell
        }
    }

    /// Returns the full scripts for the sandbox.
    pub fn get_full_scripts(&self) -> Cow<HashMap<String, String>> {
        if self.scripts.contains_key("start") {
            Cow::Borrowed(&self.scripts)
        } else {
            let mut scripts = self.scripts.clone();
            scripts.insert("start".to_string(), self.shell.clone());
            Cow::Owned(scripts)
        }
    }
}

impl SandboxNetworkConfig {
    /// Returns the default network reach configuration.
    pub fn default_reach() -> Option<SandboxNetworkReach> {
        Some(SandboxNetworkReach::Local)
    }
}

impl GroupNetworkConfig {
    /// Returns the default network reach configuration.
    pub fn default_reach() -> Option<SandboxNetworkReach> {
        Some(SandboxNetworkReach::Local)
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
    pub fn requires(mut self, requires: impl IntoIterator<Item = Require>) -> Self {
        self.requires = Some(requires.into_iter().collect());
        self
    }

    /// Sets the builds to run
    pub fn builds(mut self, builds: impl IntoIterator<Item = Build>) -> Self {
        self.builds = Some(builds.into_iter().collect());
        self
    }

    /// Sets the sandboxes to run
    pub fn sandboxes(mut self, sandboxes: impl IntoIterator<Item = Sandbox>) -> Self {
        self.sandboxes = Some(sandboxes.into_iter().collect());
        self
    }

    /// Sets the groups to run the sandboxes in
    pub fn groups(mut self, groups: impl IntoIterator<Item = Group>) -> Self {
        self.groups = Some(groups.into_iter().collect());
        self
    }

    /// Builds the Monocore configuration with validation
    pub fn build(self) -> MonocoreResult<MonocoreConfig> {
        let monocore = self.build_unchecked();
        monocore.validate()?;
        Ok(monocore)
    }

    /// Builds the Monocore configuration without validation
    pub fn build_unchecked(self) -> MonocoreConfig {
        MonocoreConfig {
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

fn serialize_path_map<S>(
    map: &HashMap<String, Utf8UnixPathBuf>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let mut map_ser = serializer.serialize_map(Some(map.len()))?;
    for (k, v) in map {
        map_ser.serialize_entry(k, v.as_str())?;
    }
    map_ser.end()
}

fn deserialize_path_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, Utf8UnixPathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    HashMap::<String, String>::deserialize(deserializer).map(|string_map| {
        string_map
            .into_iter()
            .map(|(k, v)| (k, Utf8UnixPathBuf::from(v)))
            .collect()
    })
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

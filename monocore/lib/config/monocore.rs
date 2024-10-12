//! Monocore configuration types and helpers.

use std::collections::HashMap;

use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};
use structstruck::strike;
use typed_builder::TypedBuilder;

use crate::error::MonocoreResult;

use super::{PathPair, PortPair};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

strike! {
    /// The monocore configuration.
    #[strikethrough[derive(Debug, Deserialize, Serialize, TypedBuilder, PartialEq, Getters, Setters)]]
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    pub struct Monocore {
        /// The services to run.
        #[serde(rename = "service")]
        services: Vec<Service>,

        /// The volumes to mount.
        #[serde(rename = "volume", skip_serializing_if = "Vec::is_empty", default)]
        volumes: Vec<Volume>,

        /// The networks to connect to.
        #[serde(rename = "network", skip_serializing_if = "Vec::is_empty", default)]
        networks: Vec<
            /// The network to connect to.
            pub struct Network {
                /// The name of the network.
                name: String,

                /// Whether to enable IPv6.
                #[serde(skip_serializing_if = "Option::is_none", default)]
                ipv6: Option<bool>,
            }
        >,

        /// The environment groups to use.
        #[serde(rename = "env_group", skip_serializing_if = "Vec::is_empty", default)]
        env_groups: Vec<
            /// The environment group to use.
            pub struct EnvGroup {
                /// The name of the environment group.
                name: String,

                /// The environment variables.
                #[serde(skip_serializing_if = "Vec::is_empty", default)]
                envs: Vec<
                    /// The environment variable.
                    pub struct Env {
                        /// The name of the environment variable.
                        name: String,

                        /// The value of the environment variable.
                        value: String,
                    }
                >,
            }
        >,
    }
}

/// The service to run.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

        /// The volumes to mount.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        volumes: Vec<String>,

        /// The networks to connect to.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        networks: Vec<String>,

        /// The environment groups to use.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        env_groups: Vec<String>,

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
        volumes: Vec<String>,

        /// The networks to connect to.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        networks: Vec<String>,

        /// The environment groups to use.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        env_groups: Vec<String>,

        /// The services to depend on.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        depends_on: Vec<String>,

        /// The setup commands to run.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        setup: Vec<String>,

        /// The port to expose.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        port: Option<PortPair>,
    },
    /// An HTTP event handler service. It enables serverless type workloads.
    #[serde(rename = "http_handler")]
    HttpHandler {
        /// The name of the service.
        name: String,

        /// The base image to use.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        base: Option<String>,

        /// The volumes to mount.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        volumes: Vec<String>,

        /// The networks to connect to.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        networks: Vec<String>,

        /// The environment groups to use.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        env_groups: Vec<String>,

        /// The services to depend on.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        depends_on: Vec<String>,

        /// The setup commands to run.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        setup: Vec<String>,

        /// The port to expose.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        port: Option<PortPair>,
    },
}

/// The volume to mount.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Volume {
    /// The volume is created on the fly for the guest.
    New {
        /// The name of the volume.
        name: String,

        /// The guest path to mount the volume to.
        mount_path: String,

        /// The size of the volume in MiB.
        size: u64,

        /// Whether the volume is to be deleted when the service is stopped.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        ephemeral: Option<bool>,
    },
    /// The volume is mapped from the host.
    Mapped {
        /// The name of the volume.
        name: String,

        /// The path to mount the volume from.
        path: PathPair,

        /// Whether the volume is to be deleted when the service is stopped.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        ephemeral: Option<bool>,
    },
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Monocore {
    /// Validates the configuration.
    pub fn validate(&self) -> MonocoreResult<()> {
        Ok(())
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
        type = "precursor"
        name = "precursor"
        base = "ubuntu:24.04"
        volumes = ["pre", "main"]
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
        volumes = ["main"]
        networks = ["main"]
        env_groups = ["main"]
        depends_on = ["precursor"]
        setup = [
            "cd /main"
        ]
        port = "3000:3000"
        scripts = { start = "./monocore" }

        [[volume]]
        name = "main"
        mount_path = "/main"
        size = 100

        [[volume]]
        name = "pre"
        path = "project:./"
        ephemeral = true

        [[network]]
        name = "main"
        ipv6 = true

        [[env_group]]
        name = "main"
        envs = [
            { name = "LOG_LEVEL", value = "info" },
            { name = "MONO_DATA_DIR", value = "/main" },
        ]
        "#;

        let config: Monocore = toml::from_str(config)?;

        tracing::info!("config: {:?}", config);

        assert_eq!(
            config.services,
            vec![
                Service::Precursor {
                    name: "precursor".to_string(),
                    base: Some("ubuntu:24.04".to_string()),
                    volumes: vec!["pre".to_string(), "main".to_string()],
                    networks: vec![],
                    env_groups: vec![],
                    depends_on: vec![],
                    setup: vec![
                        "apt update && apt install -y curl".to_string(),
                        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
                            .to_string(),
                        "cd /project && cargo build --release".to_string(),
                        "cp target/release/monocore /main/monocore".to_string(),
                    ],
                    port: None,
                },
                Service::Default {
                    name: "server".to_string(),
                    base: Some("ubuntu:24.04".to_string()),
                    volumes: vec!["main".to_string()],
                    networks: vec!["main".to_string()],
                    env_groups: vec!["main".to_string()],
                    depends_on: vec!["precursor".to_string()],
                    setup: vec!["cd /main".to_string()],
                    scripts: HashMap::from([("start".to_string(), "./monocore".to_string())]),
                    port: Some(PortPair::Same(3000)),
                    workdir: None,
                    command: None,
                }
            ]
        );

        assert_eq!(
            config.volumes,
            vec![
                Volume::New {
                    name: "main".to_string(),
                    mount_path: "/main".to_string(),
                    size: 100,
                    ephemeral: None,
                },
                Volume::Mapped {
                    name: "pre".to_string(),
                    path: "project:./".parse()?,
                    ephemeral: Some(true),
                },
            ]
        );

        assert_eq!(
            config.networks,
            vec![Network {
                name: "main".to_string(),
                ipv6: Some(true),
            }]
        );

        assert_eq!(
            config.env_groups,
            vec![EnvGroup {
                name: "main".to_string(),
                envs: vec![
                    Env {
                        name: "LOG_LEVEL".to_string(),
                        value: "info".to_string(),
                    },
                    Env {
                        name: "MONO_DATA_DIR".to_string(),
                        value: "/main".to_string(),
                    },
                ],
            }]
        );

        Ok(())
    }
}

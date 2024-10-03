//! Monocore configuration types and helpers.

use serde::{Deserialize, Serialize};
use structstruck::strike;
use typed_builder::TypedBuilder;

use crate::error::MonocoreResult;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

strike! {
    /// The monocore configuration.
    #[strikethrough[derive(Debug, Deserialize, Serialize, TypedBuilder)]]
    pub struct MonocoreConfig {
        /// The services to run.
        #[serde(rename = "service")]
        services: Vec<struct ServiceConfig {
            /// The name of the service.
            name: String,

            /// The base image to use.
            base: String,

            /// The volumes to mount.
            volumes: Vec<String>,

            /// The networks to connect to.
            networks: Vec<String>,

            /// The environment groups to use.
            env_groups: Vec<String>,

            /// The setup commands to run.
            setup: Vec<String>,

            /// The command to run.
            run: String,

            /// The project path.
            project_path: struct ProjectPath {
                host: String,
                container: String,
            },

            /// The HTTP configuration.
            http: struct HttpConfig {
                /// The port to expose.
                port: struct Port {
                    host: u16,
                    container: u16,
                },

                /// Whether the service is serverless.
                serverless: bool,

                /// The URL prefix.
                url_prefix: String,
            },
        }>,

        /// The volumes to mount.
        #[serde(rename = "volume")]
        volumes: Vec<struct VolumeConfig {
            /// The name of the volume.
            name: String,

            /// The path to mount.
            path: struct Path {
                /// The host path.
                host: String,

                /// The container path.
                container: String,
            },
        }>,

        /// The networks to connect to.
        #[serde(rename = "network")]
        networks: Vec<struct NetworkConfig {
            /// The name of the network.
            name: String,

            /// Whether to enable IPv6.
            ipv6: bool,
        }>,

        /// The environment groups to use.
        #[serde(rename = "env_group")]
        env_groups: Vec<struct EnvGroupConfig {
            /// The name of the environment group.
            name: String,

            /// The environment variables.
            envs: Vec<struct Env {
                /// The name of the environment variable.
                name: String,

                /// The value of the environment variable.
                value: String,
            }>,
        }>,
    }
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MonocoreConfig {
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
            name = "server"
            base = "ubuntu:24.04"
            volumes = ["main"]
            networks = ["main"]
            env_groups = ["main"]
            setup = [
                "apt update && apt install -y curl",
                "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            ]
            run = "cargo run --bin monocore"
            project_path = { host = "monocore", container = "/data/monocore" }

            [service.http]
            port = { host = 3000, container = 3000 }
            serverless = true
            url_prefix = "/api"

            [[volume]]
            name = "main"
            path = { host = "/data", container = "/" }

            [[network]]
            name = "main"
            ipv6 = true

            [[env_group]]
            name = "main"
            envs = [
                { name = "LOG_LEVEL", value = "info" },
                { name = "MONO_DATA_DIR", value = "/data" },
            ]
            "#;

        let config: MonocoreConfig = toml::from_str(config)?;

        println!("{:?}", config);

        Ok(())
    }
}

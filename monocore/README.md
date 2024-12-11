<div align="center">
  <a href="https://github.com/appcypher/monocore" target="_blank">
    <img src="https://raw.githubusercontent.com/appcypher/monocore/main/assets/monocore-thick-line-purple-gradient.svg" alt="monocore logo" width="100"></img>
  </a>

  <h1>monocore</h1>

  <p>
    <a href="https://discord.gg/T95Y3XnEAK">
      <img src="https://img.shields.io/static/v1?label=Discord&message=join%20us!&color=mediumslateblue" alt="Discord">
    </a>
    <a href="https://github.com/appcypher/monocore/actions?query=">
      <img src="https://github.com/appcypher/monocore/actions/workflows/tests_and_checks.yml/badge.svg" alt="Build Status">
    </a>
    <a href="https://crates.io/crates/monocore">
      <img src="https://img.shields.io/crates/v/monocore?label=crates" alt="Monocore Crate">
    </a>
    <a href="https://docs.rs/monocore">
      <img src="https://img.shields.io/static/v1?label=Docs&message=docs.rs&color=blue" alt="Monocore Docs">
    </a>
    <a href="https://github.com/appcypher/monocore/blob/main/LICENSE">
      <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License">
    </a>
  </p>
</div>

**`monocore`** is the engine behind the monocore platform, providing a robust foundation for running AI workloads in isolated microVMs. It handles everything from VM lifecycle management to OCI image distribution, making it easy to deploy and orchestrate code sandboxes securely.

> [!WARNING]
> This project is in early development and is not yet ready for production use.

## Table of Contents

- [Overview](#overview)
- [Library Usage](#library-usage)
- [Getting Started](#getting-started)
- [Features](#features)
- [Architecture](#architecture)
- [Development](#development)
- [License](#license)

## Overview

When developing AI agents that execute code, you need a fast development cycle:

- Docker containers? Limited isolation for untrusted code
- Traditional VMs? Minutes to start up, heavy resource usage
- Direct execution? Risky for your development machine
- Cloud sandboxes? Great for production, but slow for rapid iteration

monocore provides:
- 🔒 True VM-level isolation
- ⚡ Millisecond startup times
- 🎯 Simple REST API
- 📦 Works with standard container images
- 🔧 Full resource control
- 💻 Perfect for local development

## Library Usage

### Basic MicroVM
```rust
use monocore::vm::MicroVm;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build the MicroVm
    let vm = MicroVm::builder()
        .root_path("/path/to/rootfs")  // Path to rootfs
        .ram_mib(512)
        .exec_path("/bin/true")  // Simple no-op command
        .build()?;

    // Start the MicroVm
    tracing::info!("Starting MicroVm...");
    vm.start()?;
    Ok(())
}
```

### Service Orchestration
```rust
use monocore::{
    config::{Group, Monocore, Service},
    orchestration::Orchestrator,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a simple service
    let service = Service::builder_default()
        .name("app")
        .base("alpine:latest")
        .ram(512)
        .group("main")  // Group name is required
        .command("/bin/sleep")  // Example command
        .args(["infinity"])     // Run indefinitely
        .build();

    // Create the main group
    let main_group = Group::builder()
        .name("main")
        .build();

    let config = Monocore::builder()
        .services(vec![service])
        .groups(vec![main_group])
        .build()?;

    // Create orchestrator with log retention
    let mut orchestrator = Orchestrator::with_log_retention_policy(
        "/path/to/oci_dir",
        "/path/to/supervisor",
        LogRetentionPolicy::with_max_age_weeks(1),
    ).await?;

    // Start the service
    orchestrator.up(config).await?;

    Ok(())
}
```

## Getting Started

### Installation

**Quick Install:**
```bash
curl -sSfL https://install.monocore.dev | sh
```

This will install both the `monocore` command and its alias `mc`.

**System Requirements:**

<details>
<summary><b>Linux</b></summary>

- KVM-enabled Linux kernel (check with `ls /dev/kvm`)
- User must be in the `kvm` group (add with `sudo usermod -aG kvm $USER`)
</details>

<details>
<summary><b>macOS</b></summary>

- Apple Silicon (ARM64) only
- macOS 10.15 (Catalina) or later for Hypervisor.framework support
</details>

<details>
<summary><b>Windows</b></summary>

> Coming soon!
</details>


**Manual Build:**
```bash
# Clone the repository
git clone https://github.com/appcypher/monocore.git
cd monocore

# Build and install (installs to /usr/local/bin)
make monocore && sudo make install
```

### Basic Usage

1. Create a configuration file:
```toml
# monocore.toml
[[service]]
name = "sh-counter"
base = "alpine:latest"
ram = 256
cpus = 1
group = "demo"
command = "/bin/sh"
args = ["-c", "for i in $(seq 1 10); do echo $i; sleep 2; done"]

[[service]]
name = "python-counter"
base = "python:3.11-slim"
ram = 256
cpus = 1
group = "demo"
command = "/usr/local/bin/python3"
args = [
    "-c",
    "import time; count=0; [print(f'Count: {count+1}') or time.sleep(2) or (count:=count+1) for _ in range(10)]",
]

[[group]]
name = "demo"
local_only = true
```

2. Manage your services:
```bash
# Start services
monocore up -f monocore.toml

# View status
monocore status

# Stop services
monocore down

# Remove services
monocore remove -g main
```

3. Run in server mode:
```bash
# Start the REST API server (default port: 3456)
monocore serve --port 3456

# Or use the default port
monocore serve
```

For more CLI options:
```bash
monocore --help
```

### Configuration Schema

The `monocore.toml` configuration file supports the following structure:

```toml
# Service Definition
[[service]]
name = "service-name"          # Required: Name of the service
base = "image:tag"             # Optional: Base OCI image to use
group = "group-name"           # Optional: Group this service belongs to
command = "/path/to/binary"    # Optional: Command to run
args = ["arg1", "arg2"]        # Optional: Arguments for the command
cpus = 1                       # Optional: Number of vCPUs (default: 1)
ram = 1024                     # Optional: RAM in MiB (default: 1024)
workdir = "/app"               # Optional: Working directory
port = "8080:80"               # Optional: Port mapping (host:guest)
volumes = ["/host:/guest"]     # Optional: Volume mappings
envs = ["KEY=value"]           # Optional: Environment variables
depends_on = ["other-service"] # Optional: Service dependencies
group_envs = ["prod"]          # Optional: Environment variables for the group
group_volumes = [              # Optional: Volume mappings for the group
  {
    name = "shared-data",      # Required: Name of the volume
    path = "/data:/data"       # Required: Path mapping (host:guest)
  }
]

# Group Definition
[[group]]
name = "group-name"            # Required: Name of the group
local_only = true              # Optional: Restrict connection to local network (default: true)

# Group volume definition
[[group.volume]]
name = "shared-data"           # Required: Name of the volume
path = "/data"                 # Required: Base path on host system

# Group environment variables
[[group.env]]
name = "prod"                  # Required: Name of the environment group
envs = ["API_KEY=value"]       # Optional: Environment variables
```

#### Volume Mappings
Volumes can be specified in two formats:
- Single path (`/data`): Uses the same path on both host and guest
- Path pair (`/host:/guest`): Maps host path to a different guest path

#### Port Mappings
Ports can be specified in two formats:
- Single port (`8080`): Uses the same port on both host and guest
- Port pair (`8080:80`): Maps host port to a different guest port

#### Service Groups
Services can be organized into groups for:
- Shared volume definitions
- Common environment variables
- Network isolation (when `local_only = true`)
- Resource management

#### Dependencies
- Services can specify dependencies using `depends_on`
- Maximum dependency chain length is 32
- Services are started in dependency order

### REST API

When running in server mode, monocore provides a REST API for managing services:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/up`    | POST   | Start services defined in config |
| `/down`  | POST   | Stop running services |
| `/status`| GET    | Get status of all services |
| `/remove`| POST   | Remove service files |

Example API usage:

```bash
# Start services
curl -X POST http://localhost:3456/up \
  -H "Content-Type: application/json" \
  -d @monocore.json

# Get service status
curl http://localhost:3456/status

# Stop services in a group
curl -X POST http://localhost:3456/down \
  -H "Content-Type: application/json" \
  -d '{"group": "main"}'

# Remove services
curl -X POST http://localhost:3456/remove \
  -H "Content-Type: application/json" \
  -d '{"services": ["counter", "date-service"]}'
```

## Features

### Secure Isolation
- Isolated microVM environments for each service
- Resource constraints and limits enforcement
- Network isolation between service groups
- Perfect for running untrusted AI-generated code
- Full system call isolation

### Efficient Runtime
- Fast microVM provisioning and startup
- Millisecond-level boot times
- Minimal resource overhead
- Optimized layer caching and sharing
- Memory-efficient design

### OCI Integration
- Pull images from any OCI-compliant registry
- Smart layer management and deduplication
- Local image caching for faster startups
- Support for standard container images
- Seamless Docker compatibility

### Service Orchestration
- Dependency-aware service scheduling
- Health monitoring and automatic recovery
- Log rotation with configurable retention
- Resource usage tracking
- Group-based service management

## Architecture

### Directory Structure

Monocore maintains its state in `~/.monocore`:

```mermaid
graph TD
    monocore_root[~/.monocore] --> monoimage[monoimage/]
    monoimage --> monoimage_repo[repo/]
    monoimage_repo --> monoimage_cid["[repo-name]__[tag].cid"]
    monoimage --> monoimage_layer[layer/]

    monocore_root --> oci[oci/]
    oci --> oci_repo[repo/]
    oci_repo --> oci_tag["[repo-name]__[tag]/"]
    oci_tag --> oci_config[config.json]
    oci_tag --> oci_manifest[manifest.json]
    oci_tag --> oci_index[index.json]
    oci --> oci_layer[layer/]
    oci_layer --> oci_layer_hash["[hash]"]

    monocore_root --> rootfs[rootfs/]
    rootfs --> rootfs_service[service/]
    rootfs_service --> rootfs_service_rootfs["[service-name]/"]
    rootfs --> rootfs_ref[reference/]
    rootfs_ref --> rootfs_ref_repo["[repo-name]__[tag]/"]
    rootfs_ref_repo --> rootfs_ref_repo_merged[merged/]

    monocore_root --> service[service/]
    service --> service_info["[service-name]/"]
    service_info --> service_json[service.json]
    service_info --> group_json[group.json]

    monocore_root --> run[run/]
    run --> run_service["[service-name]__[pid].json"]

    monocore_root --> log[log/]
    log --> log_stderr["[service-name].stderr.log"]
    log --> log_stdout["[service-name].stdout.log"]
```

## Development

For development setup and building from source, please visit the [root of the project repository](https://github.com/appcypher/monocore).

## License

This project is licensed under the [Apache License 2.0](./LICENSE).

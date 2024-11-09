<div align="center">
  <h1>monocore</h1>
  <p>Core library powering secure, sandboxed AI environments</p>

  <p>
    <a href="https://github.com/appcypher/monocore/actions?query=">
      <img src="https://github.com/appcypher/monocore/actions/workflows/tests_and_checks.yml/badge.svg" alt="Build Status">
    </a>
    <a href="https://github.com/appcypher/monocore/blob/main/LICENSE">
      <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License">
    </a>
  </p>
</div>

**`monocore`** is the engine behind the monocore platform, providing a robust foundation for running AI workloads in isolated microVMs. It handles everything from VM lifecycle management to OCI image distribution, making it easy to deploy and orchestrate AI agents securely.

> [!WARNING]
> This project is in early development and is not yet ready for production use.

##

## Outline

- [Features](#features)
- [Directory Structure](#directory-structure)
- [Quick Start](#quick-start)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Features

### 🔒 Secure by Design
- Isolated microVM environments for each service
- Resource constraints and limits enforcement
- Network isolation between service groups

### 🏃 Efficient Runtime
- Fast microVM provisioning and startup
- Minimal resource overhead
- Optimized layer caching and sharing

### 📦 OCI Integration
- Pull images from any OCI-compliant registry
- Smart layer management and deduplication
- Local image caching for faster startups

### 🎯 Service Orchestration
- Dependency-aware service scheduling
- Health monitoring and automatic recovery
- Log rotation with configurable retention

## Directory Structure

The library maintains its state in `~/.monocore`:

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

    monocore_root --> microvm[microvm/]
    microvm --> microvm_tag["[repo-name]__[tag]/"]
    microvm --> microvm_instance["[repo-name]__[tag]__[uuid]/"]
    microvm_tag --> microvm_service[service.toml]
    microvm_tag --> microvm_cid["[repo-name]__[tag].cid"]
    microvm_tag --> microvm_rootfs[rootfs/]
    microvm_instance --> microvm_instance_rootfs[rootfs/]

    monocore_root --> run[run/]
    run --> run_service["[service-name]__[supervisor-pid].json"]

    monocore_root --> log[log/]
    log --> log_stderr["[service-name].stderr.log"]
    log --> log_stdout["[service-name].stdout.log"]
```

## Quick Start

### Basic MicroVM
```rust
use monocore::vm::MicroVm;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let vm = MicroVm::builder()
        .root_path("/path/to/rootfs")
        .ram_mib(512)
        .exec_path("/bin/echo")
        .args(["Hello from microVM!"])
        .build()?;

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
    let service = Service::builder_default()
        .name("ai-agent")
        .base("alpine:latest")
        .ram(512)
        .build();

    let config = Monocore::builder()
        .services(vec![service])
        .groups(vec![Group::builder().name("agents").build()])
        .build()?;

    let mut orchestrator = Orchestrator::new("/path/to/rootfs", "/path/to/supervisor").await?;
    orchestrator.up(config).await?;

    Ok(())
}
```

## Development

### Prerequisites
- Rust toolchain (1.75+)
- libkrun development files

### Building
```bash
# Build the library
cargo build

# Run tests
cargo test

# Try an example
cargo run --example microvm_shell
```

### Examples
The `examples/` directory showcases key features:
- `microvm_shell.rs`: Basic microVM usage
- `oci_pull.rs`: Image pulling and caching
- `orchestration_basic.rs`: Service orchestration
- `orchestration_load.rs`: Load testing

## Contributing

Please read our [Contributing Guide](../../CONTRIBUTING.md) for details on:
- Code style and conventions
- Commit message format
- Pull request process
- Testing requirements

## License

This project is licensed under the [Apache License 2.0](../../LICENSE).

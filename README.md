<div align="center">
  <a href="https://github.com/appcypher/monocore" target="_blank">
    <img src="https://raw.githubusercontent.com/appcypher/monocore/main/assets/monocore-thick-line-purple-gradient.svg" alt="monocore logo" width="100"></img>
  </a>

  <h1 align="center">monocore</h1>

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

> [!WARNING]
> This project is in early development and is not yet ready for production use.

Building AI agents that write and execute code? You'll need a secure sandbox.

**monocore** provides instant, secure VMs for your AI agents to:

- Generate charts and visualizations
- Run data analysis scripts
- Test generated code
- Execute system commands
- Access development tools

All while keeping your system safe through VM-level isolation.

## Why monocore?

When developing AI agents that execute code, you need a fast development cycle:

- Docker containers? Limited isolation for untrusted code
- Traditional VMs? Minutes to start up, heavy resource usage
- Direct execution? Risky for your development machine
- Cloud sandboxes? Great for production, but slow for rapid iteration

monocore gives you:

- 🔒 True VM-level isolation
- ⚡ Millisecond startup times
- 🎯 Simple REST API
- 📦 Works with standard container images
- 🔧 Full resource control
- 💻 Perfect for local development

Develop and test locally with instant feedback, then deploy to production with confidence.

## Getting Started

### Installation

```sh
curl -sSfL https://install.monocore.dev | sh
```

This will install both the `monocore` command and its alias `mc`.

### System Requirements

#### Linux

- KVM-enabled Linux kernel (check with `ls /dev/kvm`)
- User must be in the `kvm` group (add with `sudo usermod -aG kvm $USER`)

#### macOS

- macOS 10.15 (Catalina) or later for Hypervisor.framework support

### Quick Start

1. **Start the sandbox server**

   ```sh
   # Start the server on port 3456
   mc serve --port 3456
   ```

2. **Define your sandboxes**

   ```toml
   # monocore.toml

   # Python sandbox for data visualization
   [[service]]
   name = "python-sandbox"
   base = "python:3.11-slim"
   ram = 512
   cpus = 1
   group = "sandboxes"
   command = "python"
   args = ["-c", "print('Hello from Python!')"]

   # Node.js sandbox for code execution
   [[service]]
   name = "node-sandbox"
   base = "node:18-slim"
   ram = 256
   cpus = 1
   group = "sandboxes"
   command = "node"
   args = ["-e", "console.log('Hello from Node!')"]

   # Define security group
   [[group]]
   name = "sandboxes"
   local_only = true  # Restrict network access
   ```

3. **Manage your sandboxes**

   ```sh
   # Pull sandbox images
   mc pull python:3.11-slim
   mc pull node:18-slim

   # Start sandboxes
   mc up -f monocore.toml

   # Check status
   mc status

   # Stop specific services
   mc down --group sandboxes

   # Stop all services
   mc down

   # Remove services
   mc remove python-sandbox node-sandbox
   ```

### CLI Reference

```sh
# General commands
mc --help                    # Show help
mc --version                 # Show version

# Service management
mc up -f monocore.toml      # Start services
mc up --group mygroup       # Start specific group
mc down                     # Stop all services
mc down --group mygroup     # Stop specific group
mc status                   # Show service status
mc remove service-name      # Remove service

# Image management
mc pull image:tag           # Pull container image

# Server mode
mc serve --port 3456        # Start API server
```

### REST API

When running in server mode, monocore provides a REST API for programmatic control:

```sh
# Launch sandboxes
curl -X POST http://localhost:3456/up \
  -H "Content-Type: application/json" \
  -d @monocore.example.json

# Check status and metrics
curl http://localhost:3456/status | jq

# Stop all services
curl -X POST http://localhost:3456/down

# Stop specific group
curl -X POST http://localhost:3456/down \
  -H "Content-Type: application/json" \
  -d '{"group": "app"}'

# Remove services
curl -X POST http://localhost:3456/remove \
  -H "Content-Type: application/json" \
  -d '{"services": ["timer"]}'
```

## Features in Action

- **Secure Code Execution**: Run untrusted code in isolated environments
- **Resource Limits**: Control CPU, memory, and execution time
- **Network Control**: Restrict or allow network access per sandbox
- **Environment Control**: Pass data and configuration safely
- **Status Monitoring**: Track execution state and resource usage
- **Simple Integration**: RESTful API for easy automation

## Development

### Prerequisites

#### Linux Build Dependencies

```sh
# Ubuntu/Debian:
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev flex bison bc libelf-dev python3-pyelftools patchelf

# Fedora:
sudo dnf install build-essential pkg-config libssl-dev flex bison bc libelf-dev python3-pyelftools patchelf
```

#### macOS Build Dependencies

- [Homebrew][brew_home]

```sh
# Build
make build
make install
```

## Documentation

- [Detailed Features](monocore/README.md#features)
- [Architecture](monocore/README.md#architecture)
- [API Examples](monocore/README.md#api-examples)
- [Development Guide](monocore/README.md#development)

## License

This project is licensed under the [Apache License 2.0](./LICENSE).

[libkrun-repo]: https://github.com/containers/libkrun
[brew_home]: https://brew.sh/
[rustup_home]: https://rustup.rs/
[git_home]: https://git-scm.com/

<div align="center">
  <a href="https://github.com/appcypher/monocore" target="_blank">
    <img src="https://raw.githubusercontent.com/appcypher/monocore/main/assets/monocore-thick-line-purple-gradient.svg" alt="monocore logo" width="100"></img>
  </a>

  <h1 align="center">monocore</h1>

  <p>
    <a href="https://github.com/appcypher/monocore/actions?query=">
      <img src="https://github.com/appcypher/monocore/actions/workflows/tests_and_checks.yml/badge.svg" alt="Build Status">
    </a>
    <a href="https://github.com/appcypher/monocore/blob/main/LICENSE">
      <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License">
    </a>
  </p>
</div>

> [!WARNING]
> This project is in early development and is not yet ready for production use.

Building AI agents that can write and execute code? You'll need a secure sandbox.

**monocore** provides instant, secure VMs for your AI agents to:
- Generate charts and visualizations
- Run data analysis scripts
- Test generated code
- Execute system commands
- Access development tools

All while keeping your system safe through VM-level isolation.

```sh
# Start the sandbox orchestrator
monocore serve

# Your AI agent can now safely:
curl -X POST http://localhost:3456/up -d @config.json   # Launch secure VMs
curl http://localhost:3456/status                       # Monitor execution
curl -X POST http://localhost:3456/down                 # Clean up when done
```

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

### System Requirements

#### Linux
- KVM-enabled Linux kernel (check with `ls /dev/kvm`)
- User must be in the `kvm` group (add with `sudo usermod -aG kvm $USER`)

#### macOS
- macOS 10.15 (Catalina) or later for Hypervisor.framework support

1. **Install monocore**

   ```sh
   git clone https://github.com/appcypher/monocore
   cd monocore
   make build
   make install
   ```

2. **Start the sandbox server**

   ```sh
   # Start the server on port 3456
   monocore serve --port 3456
   ```

   Your AI agent now has a secure execution environment for its code execution needs!

3. **Define your sandboxes**

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
   args = ["-c", "import matplotlib.pyplot as plt; plt.plot([1, 2, 3]); plt.savefig('chart.png')"]
   env = [
     { key = "PYTHONUNBUFFERED", value = "1" }
   ]

   # Node.js sandbox for code execution
   [[service]]
   name = "node-sandbox"
   base = "node:18-slim"
   ram = 256
   cpus = 1
   group = "sandboxes"
   command = "node"
   args = ["-e", "console.log('Hello from secure sandbox!')"]

   # Define security group
   [[group]]
   name = "sandboxes"
   local_only = true  # Restrict network access
   ```

4. **Manage your sandboxes**

   Using the CLI:
   ```sh
   # Pull sandbox images
   monocore pull python:3.11-slim
   monocore pull node:18-slim

   # Start sandboxes
   monocore up -f monocore.toml

   # Monitor sandbox execution
   monocore status
   ```

   Or via the REST API:
   ```sh
   # Launch a sandbox
   curl -X POST http://localhost:3456/up \
     -H "Content-Type: application/json" \
     -d @monocore.json

   # Check execution status
   curl http://localhost:3456/status | jq '.services[] | {name, status, metrics}'
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

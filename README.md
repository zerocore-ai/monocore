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

Building AI agents that write and execute code? You'll need a secure sandbox.

**monocore** provides instant, secure VMs for your AI agents to:

- Generate visualizations and charts
- Run data analysis scripts
- Execute system commands safely
- Create and test web applications
- Run automated browser tasks
- Perform complex calculations

All while keeping your system safe through VM-level isolation.

> [!WARNING]
> This project is in early development and is not yet ready for production use.

## 🤔 Why monocore?

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

## 🚀 Getting Started

### Installation

```sh
curl -sSfL https://install.monocore.dev | sh
```

This will install both the `monocore` command and its alias `mc`.

### System Requirements

<details>
<summary><img src="https://cdn.simpleicons.org/linux/FFCC00" height="10"/> <b>Linux</b></summary>

- KVM-enabled Linux kernel (check with `ls /dev/kvm`)
- User must be in the `kvm` group (add with `sudo usermod -aG kvm $USER`)
</details>

<details>
<summary><img src="https://cdn.simpleicons.org/apple/999999" height="10"/> <b>macOS</b></summary>

- Apple Silicon (ARM64) only
- macOS 10.15 (Catalina) or later for Hypervisor.framework support
</details>

<details>
<summary><img src="https://upload.wikimedia.org/wikipedia/commons/thumb/8/87/Windows_logo_-_2021.svg/1024px-Windows_logo_-_2021.svg.png" height="10"/> <b>Windows</b></summary>

> Coming soon!

</details>

### Quick Start

1. **Define your sandboxes**

   ```toml
   # monocore.toml
   [[service]]
    name = "sh-counter"
    base = "alpine:latest"
    ram = 256
    cpus = 1
    group = "demo"
    command = "/bin/sh"
    args = ["-c", "for i in $(seq 1 20); do echo $i; sleep 2; done"]

    [[service]]
    name = "python-counter"
    base = "python:3.11-slim"
    ram = 256
    cpus = 1
    group = "demo"
    command = "/usr/local/bin/python3"
    args = [
        "-c",
        "import time; count=0; [print(f'Count: {count+1}') or time.sleep(2) or (count:=count+1) for _ in range(20)]",
    ]

    [[group]]
    name = "demo"
    local_only = true
   ```

2. **Manage your sandboxes**

   Start sandboxes:

   ```sh
   mc up -f monocore.toml
   ```

   Check status:

   ```sh
   mc status
   ```

   Check logs:

   ```sh
   mc log sh-counter --no-pager -n 10
   ```

   Stop specific services:

   ```sh
   mc down --group demo
   ```

   Stop all services:

   ```sh
   mc down
   ```

   Remove services:

   ```sh
   mc remove timer counter
   ```

   For a complete list of commands and options, use:

   ```sh
   mc --help
   ```

### REST API

Start the server (default port: 3456):

```sh
mc serve
```

Launch sandboxes:

```sh
curl -X POST http://localhost:3456/up \
  -H "Content-Type: application/json" \
  -d @monocore.example.json
```

Check status and metrics:

```sh
curl http://localhost:3456/status | jq
```

Stop all services:

```sh
curl -X POST http://localhost:3456/down
```

Stop specific group:

```sh
curl -X POST http://localhost:3456/down \
  -H "Content-Type: application/json" \
  -d '{"group": "app"}'
```

Remove services:

```sh
curl -X POST http://localhost:3456/remove \
  -H "Content-Type: application/json" \
  -d '{"services": ["timer"]}'
```

## Troubleshooting

#### Service Won't Start

- **macOS**: MicroVMs require at least 256 MiB of RAM to start properly. Setting lower values will cause silent failures.
  ```toml
  [[service]]
  name = "my-service"
  ram = 256  # Minimum recommended for macOS
  ```

## 💻 Development

For development, you'll need to build monocore from source.

### Prerequisites

<details>
<summary><img src="https://cdn.simpleicons.org/linux/FFCC00" height="10"/> <b>Linux Requirements</b></summary>

```sh
# Ubuntu/Debian:
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev flex bison bc libelf-dev python3-pyelftools patchelf

# Fedora:
sudo dnf install build-essential pkg-config libssl-dev flex bison bc libelf-dev python3-pyelftools patchelf
```

</details>

<details>
<summary><img src="https://cdn.simpleicons.org/apple/999999" height="10"/> <b>macOS Requirements</b></summary>

Make sure you have [Homebrew](https://brew.sh/) installed, then:

```sh
brew tap slp/krun
brew install krunvm
```

</details>

### Setup

1. Clone the repository:
   ```sh
   git clone https://github.com/appcypher/monocore.git
   cd monocore
   ```

2. Install pre-commit hooks:
   ```sh
   pip install pre-commit
   pre-commit install
   ```

### Build

```sh
make build
make install
```

### Release Process

When a release-please PR is created, the following manual changes need to be made before merging:

1. **Update Internal Dependencies**: In the root `Cargo.toml`, ensure that any internal crate dependencies use the new release version being created. For example:

   ```toml
   [dependencies]
   monoutils-x = { version = "0.2.0", path = "monoutils-x" }  # Update this version
   ```

2. **Update Install Script Version**: In `install_monocore.sh`, update the version number to match the new release version.

These changes are not automatically handled by release-please and must be made manually before merging the release PR.

## 📚 Documentation

- [Detailed Features](monocore/README.md#features)
- [Architecture](monocore/README.md#architecture)
- [API Examples](monocore/README.md#api-examples)
- [Development Guide](monocore/README.md#development)

## ⚖️ License

This project is licensed under the [Apache License 2.0](./LICENSE).

[libkrun-repo]: https://github.com/containers/libkrun
[brew_home]: https://brew.sh/
[rustup_home]: https://rustup.rs/
[git_home]: https://git-scm.com/

<div align="center">
  <a href="https://github.com/appcypher/monocore" target="_blank">
    <img src="https://raw.githubusercontent.com/appcypher/monocore/main/assets/monocore-thick-line-purple-gradient.svg" alt="monocore logo" width="100"></img>
  </a>

  <h1 align="center">monocore</h1>

  <p>
    <a href="https://discord.gg/T95Y3XnEAK">
      <img src="https://img.shields.io/static/v1?label=Discord&message=join%20us!&color=mediumslateblue&logo=discord&logoColor=white" alt="Discord">
    </a>
    <a href="https://github.com/appcypher/monocore/actions?query=">
      <img src="https://github.com/appcypher/monocore/actions/workflows/tests_and_checks.yml/badge.svg" alt="Build Status">
    </a>
    <a href="https://crates.io/crates/monocore">
      <img src="https://img.shields.io/crates/v/monocore?label=crates&logo=rust" alt="Monocore Crate">
    </a>
    <a href="https://docs.rs/monocore">
      <img src="https://img.shields.io/static/v1?label=Docs&message=docs.rs&color=blue&logo=docs.rs" alt="Monocore Docs">
    </a>
    <a href="https://github.com/appcypher/monocore/blob/main/LICENSE">
      <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg?logo=apache&logoColor=white" alt="License">
    </a>
  </p>
</div>

> [!WARNING]
> This project is currently undergoing significant architectural changes and active development. As a result, comprehensive documentation and a detailed README will be provided once the core components stabilize. Stay tuned for updates.

## 🗺️ Project Roadmap

<div align="center">

**Project Status**

</div>

<div align="center">

<kbd>⬜️ Planning</kbd> <kbd>🟨 In Progress</kbd> <kbd>✅ Shipped</kbd>

</div>

<div align="center">

| Category          | Component              | Status | Description                                              |
| :---------------- | :--------------------- | :----: | :------------------------------------------------------- |
| **🎯 Core**       |
|                   | **Configuration**      |   🟨   | YAML-based sandbox and project configuration             |
|                   | • Validation           |  ⬜️   | Configuration schema validation and verification         |
|                   | • Import               |  ⬜️   | External component configuration imports                 |
|                   | **Networking**         |  ⬜️   | Sandbox network management and isolation                 |
|                   | • IP Assignment        |  ⬜️   | Subnet (10.0.0.0/8) and IP management for sandbox groups |
|                   | • Packet Filtering     |  ⬜️   | Network reach control (local/public/any/none)            |
|                   | **Orchestration**      |  ⬜️   | Sandbox lifecycle and resource management                |
|                   | • Build Steps          |  ⬜️   | Image build pipeline and artifact management             |
|                   | • Sandbox provisioning |  ⬜️   | libkrun-based microVM provisioning                       |
|                   | • Sandbox Groups       |  ⬜️   | Shared network, volume and env management                |
| **🛠️ CLI Tools**  |
|                   | **monocore CLI**       |   🟨   | Project and sandbox management interface                 |
|                   | • `init`               |  ⬜️   | Interactive project initialization                       |
|                   | • `add`                |  ⬜️   | Add sandboxes, builds, or groups to project              |
|                   | • `remove`             |  ⬜️   | Remove project components                                |
|                   | • `list`               |  ⬜️   | List sandboxes, builds, or groups                        |
|                   | • `log`                |  ⬜️   | View component logs with filtering                       |
|                   | • `tree`               |  ⬜️   | Display component layer hierarchy                        |
|                   | • `run`                |  ⬜️   | Execute defined component scripts                        |
|                   | • `start`              |  ⬜️   | Execute component start scripts                          |
|                   | • `shell`              |  ⬜️   | Interactive sandbox shell access                         |
|                   | • `tmp`                |  ⬜️   | Temporary sandbox creation from images                   |
|                   | • `install`            |  ⬜️   | Global installation of image scripts                     |
|                   | • `uninstall`          |  ⬜️   | Remove globally installed scripts                        |
|                   | • `apply`              |  ⬜️   | Apply configuration to running sandboxes                 |
|                   | • `up`                 |  ⬜️   | Start sandboxes or groups                                |
|                   | • `down`               |  ⬜️   | Stop sandboxes or groups                                 |
|                   | • `status`             |  ⬜️   | View sandbox runtime status                              |
|                   | • `clean`              |  ⬜️   | Clean sandbox and project data                           |
|                   | • `build`              |  ⬜️   | Build images from configurations                         |
|                   | • `pull`               |   🟨   | Pull OCI images from registries                          |
|                   | • `push`               |  ⬜️   | Push images to OCI registries                            |
|                   | • `self`               |  ⬜️   | Manage monocore installation and updates                 |
|                   | • `deploy`             |  ⬜️   | Cloud deployment of sandboxes                            |
|                   | • `serve`              |  ⬜️   | Run sandbox orchestration server                         |
|                   | **monofs CLI**         |   🟨   | Versioned filesystem management interface                |
|                   | • `init`               |   ✅   | Initialize versioned filesystem at mount point           |
|                   | • `tmp`                |  ⬜️   | Create temporary versioned filesystem                    |
|                   | • `clone`              |  ⬜️   | Clone existing versioned filesystem                      |
|                   | • `sync`               |  ⬜️   | Synchronize filesystems (backup/raft/crdt)               |
|                   | • `rev`                |  ⬜️   | View filesystem revision history                         |
|                   | • `tag`                |  ⬜️   | Create named tags for revisions                          |
|                   | • `checkout`           |  ⬜️   | Switch to specific revision                              |
|                   | • `diff`               |  ⬜️   | Compare filesystem revisions                             |
|                   | • `detach`             |   ✅   | Safely unmount filesystem and stop NFS server            |
| **🐋 OCI**        |
|                   | **OverlayFS**          |   ✅   | OverlayFS implementation on macOS                        |
|                   | • Core                 |   ✅   | Core implementation of the OverlayFS                     |
|                   | • NFS                  |   ✅   | Network File System server implementation                |
|                   | • NativeFS             |   ✅   | Native filesystem implementation                         |
|                   | • virtiofs             |  ⬜️   | libkrun virtiofs implementation                          |
|                   | Sandboxes Registry     |  ⬜️   | Container sandboxing registry implementation             |
|                   | Docker Registry        |   ✅   | Integration with Docker registry                         |
|                   | ghcr Registry          |  ⬜️   | Integration with GitHub Container Registry               |
|                   | Quay Registry          |  ⬜️   | Integration with Red Hat Quay registry                   |
| **📊 Web UI**     |
|                   | Desktop                |  ⬜️   | App dashboard                                            |
| **🔌 SDK**        |
|                   | Python SDK             |  ⬜️   | Sandbox orchestration with Python                        |
|                   | TypeScript SDK         |  ⬜️   | Sandbox orchestration with TypeScript                    |
| **🌍 REST API**   |
|                   | Orchestration API      |  ⬜️   | Orchestration API implementation                         |
| **📂 monofs**     |
|                   | Chunking               |   ✅   | Content-based chunking for efficient storage             |
|                   | Versioning             |   ✅   | File and directory versioning support                    |
|                   | NFS Server             |   ✅   | Network File System server implementation                |
|                   | Compression            |  ⬜️   | Data compression for storage efficiency                  |
|                   | Backup Sync            |  ⬜️   | Automated backup synchronization                         |
|                   | Raft Sync              |  ⬜️   | Distributed consensus using Raft                         |
|                   | Merkle CRDT Sync       |  ⬜️   | Conflict-free replicated data types with Merkle trees    |
|                   | E2E Encryption         |  ⬜️   | End-to-end encryption for secure storage                 |
| **⚡ Serverless** |
|                   | Legacy Support         |  ⬜️   | Serverless-like behavior for legacy applications         |
|                   | Handlers               |  ⬜️   | Function handlers and routing                            |

</div>

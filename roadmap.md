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
|                   | Dashboard                |  ⬜️   | Sandbox dashboard                                            |
| **🔌 SDK**        |
|                   | Python SDK             |  ⬜️   | Sandbox orchestration with Python                        |
|                   | TypeScript SDK         |  ⬜️   | Sandbox orchestration with TypeScript                    |
| **🌍 REST API**   |
|                   | Orchestration API      |  ⬜️   | Orchestration API implementation                         |
| **⚡ Serverless** |
|                   | Legacy Support         |  ⬜️   | Serverless-like behavior for legacy applications         |
|                   | Handlers               |  ⬜️   | Function handlers and routing                            |

</div>

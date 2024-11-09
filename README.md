<div align="center">
  <!-- <a href="https://github.com/appcypher/monocore" target="_blank">
    <img src="https://raw.githubusercontent.com/appcypher/monocore/main/assets/a_logo.png" alt="monocore Logo" width="100"></img>
  </a> -->

  <h1 align="center">monocore</h1>

  <p>
    <!-- <a href="https://crates.io/crates/monocore">
      <img src="https://img.shields.io/crates/v/monocore?label=crates" alt="Crate">
    </a> -->
    <a href="https://github.com/appcypher/monocore/actions?query=">
      <img src="https://github.com/appcypher/monocore/actions/workflows/tests_and_checks.yml/badge.svg" alt="Build Status">
    </a>
    <a href="https://github.com/appcypher/monocore/blob/main/LICENSE">
      <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License">
    </a>
    <!-- <a href="https://docs.rs/monocore">
      <img src="https://img.shields.io/static/v1?label=Docs&message=docs.rs&color=blue" alt="Docs">
    </a> -->
  </p>
</div>

**`monocore`** is your gateway to spin up AI-driven sandboxed environments in no time.

Tired of clunky cloud setups? `monocore` is an open-source, self-hostable platform that lets you orchestrate microVMs for your AI agents—right from your local dev machine. With just a single command, you're up and running.

Forget the headaches of replication. `monocore` is distributed by design, so your VMs are seamlessly taken care of, letting you focus on building and experimenting without limits.

Want the power of fly.io but with the freedom of self-hosting? That's where `monocore` comes in. No lock-ins, no unnecessary overhead—just lightweight VMs, ready when you are.

> [!WARNING]
> This project is in early development and is not yet ready for production use.

##

## Outline

- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Development

Follow these steps to set up monocore for development:

### Prerequisites

- [Git][git_home]
- [Rust toolchain][rustup_home]
- On macOS: [Homebrew][brew_home]

### Setup

1. **Clone the repository**

   ```sh
   git clone https://github.com/appcypher/monocore
   cd monocore
   ```

2. **Build libkrun**

   monocore uses a modified version of [libkrun][libkrun-repo] for its microVMs.

   ```sh
   ./build_libkrun.sh
   ```

   > **Note for macOS users:** Install `krunvm` before building libkrun:
   >
   > ```sh
   > brew tap slp/tap
   > brew install krunvm
   > ```

3. **Build and install monocore**

   ```sh
   cd monocore
   make
   sudo make install
   ```

## Contributing

1. **Read the [CONTRIBUTING.md](./CONTRIBUTING.md) file**

   This file contains information about the coding style, commit message conventions,
   and other guidelines that you should follow when contributing to monocore.

2. **Install pre-commit hooks**

   ```sh
   pre-commit install
   ```

   You will need to have `pre-commit` installed to use these hooks.

## License

This project is licensed under the [Apache License 2.0](./LICENSE).

[libkrun-repo]: https://github.com/containers/libkrun
[brew_home]: https://brew.sh/
[rustup_home]: https://rustup.rs/
[git_home]: https://git-scm.com/

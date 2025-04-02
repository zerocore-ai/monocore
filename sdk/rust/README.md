# Microsandbox Rust SDK

A minimal Rust SDK for the Microsandbox project.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
microsandbox = "0.0.1"
```

## Usage

```rust
use microsandbox::greet;

fn main() {
    // Print a greeting
    greet("World");
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/rust

# Build the project
cargo build
```

### Running Tests

```bash
cargo test
```

### Publishing to crates.io

1. Create an account on [crates.io](https://crates.io/) if you don't have one yet.

2. Log in to crates.io via Cargo:

```bash
cargo login
```

3. Publish the crate:

```bash
cargo publish
```

You will need to own the crate name on crates.io before publishing. Make sure your `Cargo.toml` contains the necessary metadata before publishing.

## License

[MIT](LICENSE)

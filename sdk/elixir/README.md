# Microsandbox Elixir SDK

A minimal Elixir SDK for the Microsandbox project.

## Installation

Add the dependency to your `mix.exs` file:

```elixir
def deps do
  [
    {:microsandbox, "~> 0.0.1"}
  ]
end
```

Then run:

```bash
mix deps.get
```

## Usage

```elixir
# Print a greeting
Microsandbox.greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/elixir

# Get dependencies
mix deps.get
```

### Running Tests

```bash
mix test
```

### Building the Documentation

```bash
mix docs
```

### Publishing to Hex.pm

1. Create an account on [Hex.pm](https://hex.pm/) if you don't have one.

2. Authenticate with Hex:

```bash
mix hex.user auth
```

3. Publish to Hex:

```bash
mix hex.publish
```

For more information, see [Publishing a Package](https://hex.pm/docs/publish).

## License

[MIT](LICENSE)

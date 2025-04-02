# Microsandbox Erlang SDK

A minimal Erlang SDK for the Microsandbox project.

## Installation

### Rebar3

Add to your `rebar.config`:

```erlang
{deps, [
    {microsandbox, "0.0.1"}
]}.
```

### Mix (for Elixir projects)

```elixir
def deps do
  [
    {:microsandbox, "~> 0.0.1"}
  ]
end
```

## Usage

```erlang
% Print a greeting
{ok, Greeting} = microsandbox:greet("World"),
io:format("~s~n", [Greeting]).
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/erlang

# Compile the project
rebar3 compile
```

### Running Tests

```bash
rebar3 eunit
```

### Building Documentation

```bash
rebar3 edoc
```

### Publishing to Hex.pm

1. Create an account on [Hex.pm](https://hex.pm/) if you don't have one.

2. Add Hex as a plugin to rebar3 in your `~/.config/rebar3/rebar.config`:
```erlang
{plugins, [rebar3_hex]}.
```

3. Configure your Hex credentials:
```bash
rebar3 hex user auth
```

4. Publish your package:
```bash
rebar3 hex publish
```

For more details, see [Publishing Packages with rebar3_hex](https://hex.pm/docs/rebar3_publish).

## License

[MIT](LICENSE)

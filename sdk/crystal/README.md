# Microsandbox Crystal SDK

A lightweight Crystal SDK for interacting with the Microsandbox service.

## Installation

1. Add the dependency to your `shard.yml`:

```yaml
dependencies:
  microsandbox:
    github: microsandbox/crystal
```

2. Run `shards install`

## Usage

```crystal
require "microsandbox"

# Simple greeting
puts Microsandbox.greet("World")
```

## Development

1. Clone this repository
2. Run `shards install`
3. Make your changes
4. Run tests with `crystal spec`

## Contributing

1. Fork it (<https://github.com/microsandbox/crystal/fork>)
2. Create your feature branch (`git checkout -b my-new-feature`)
3. Commit your changes (`git commit -am 'Add some feature'`)
4. Push to the branch (`git push origin my-new-feature`)
5. Create a new Pull Request

## License

[MIT](LICENSE)

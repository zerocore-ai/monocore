# Microsandbox Ruby SDK

A minimal Ruby SDK for the Microsandbox project.

## Installation

Add this line to your application's Gemfile:

```ruby
gem 'microsandbox'
```

And then execute:

```bash
bundle install
```

Or install it yourself as:

```bash
gem install microsandbox
```

## Usage

```ruby
require 'microsandbox'

# Print a greeting
Microsandbox.greet('World')
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/ruby

# Install dependencies
bundle install
```

### Running Tests

```bash
rake test
```

### Building the Gem

```bash
gem build microsandbox.gemspec
```

### Publishing to RubyGems

```bash
# Login to RubyGems (if not already logged in)
gem signin

# Publish the gem
gem push microsandbox-0.0.1.gem
```

Make sure you have registered for an account on [RubyGems](https://rubygems.org/) and verified your email address.

## License

[MIT](LICENSE)

# Microsandbox Nim SDK

A minimal Nim SDK for the Microsandbox project.

## Installation

```bash
nimble install microsandbox
```

Or add to your `.nimble` file:

```nim
requires "microsandbox >= 0.0.1"
```

## Usage

```nim
import microsandbox

# Print a greeting
let message = greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/nim

# Install dependencies
nimble install
```

### Testing

```bash
nimble test
```

### Building

```bash
nimble build
```

### Publishing to Nimble Package Directory

The [Nimble Package Directory](https://nimble.directory/) is the official package repository for Nim.

To publish your package:

1. Create a GitHub repository for your Nim package with the proper structure
2. Make sure your `.nimble` file is properly configured
3. Tag a release on GitHub

```bash
# Tag your release
git tag v0.0.1
git push origin v0.0.1
```

4. Submit your package to the Nimble Package Directory:

```bash
# Login to GitHub (if you haven't already)
nimble publish
```

Alternatively, you can manually add your package to the [nim-lang/packages](https://github.com/nim-lang/packages) repository by adding an entry to the `packages.json` file:

```json
{
  "name": "microsandbox",
  "url": "https://github.com/yourusername/monocore",
  "method": "git",
  "tags": ["sdk", "microsandbox"],
  "description": "A minimal Nim SDK for the Microsandbox project",
  "license": "MIT",
  "web": "https://github.com/yourusername/monocore/tree/main/sdk/nim"
}
```

## License

[MIT](LICENSE)

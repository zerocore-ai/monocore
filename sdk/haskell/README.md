# Microsandbox Haskell SDK

A minimal Haskell SDK for the Microsandbox project.

## Installation

Add to your `package.yaml` or `.cabal` file:

```yaml
dependencies:
  - microsandbox
```

Or using Cabal:

```bash
cabal install microsandbox
```

Or using Stack:

```bash
stack install microsandbox
```

## Usage

```haskell
import Microsandbox

main :: IO ()
main = do
  -- Print a greeting
  greeting <- greet "World"
  putStrLn greeting
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/haskell

# Build the package
cabal build

# Or with Stack
stack build
```

### Running Tests

```bash
cabal test

# Or with Stack
stack test
```

### Building Documentation

```bash
cabal haddock

# Or with Stack
stack haddock
```

### Publishing to Hackage

1. Create an account on [Hackage](https://hackage.haskell.org/), if you don't have one.

2. Create a source distribution:

```bash
cabal sdist
```

3. Upload to Hackage:

```bash
cabal upload dist-newstyle/sdist/microsandbox-0.0.1.tar.gz
```

4. Publish the documentation:

```bash
cabal upload --documentation --publish dist-newstyle/sdist/microsandbox-0.0.1-docs.tar.gz
```

For more information, see [Uploading to Hackage](https://hackage.haskell.org/upload).

## License

[MIT](LICENSE)

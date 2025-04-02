# Microsandbox Elm SDK

A minimal Elm SDK for the Microsandbox project.

## Installation

```bash
elm install yourusername/microsandbox
```

## Usage

```elm
import Microsandbox exposing (greet)

-- Print a greeting
main =
    greet "World"
    |> text
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/elm

# Install Elm if not already installed
npm install -g elm
```

### Running Tests

```bash
elm-test
```

### Publishing to Elm Package Repository

1. Make sure your code is in its own GitHub repository.

2. Initialize your Elm package:
```bash
elm init
```

3. Fill in the resulting `elm.json` file with your package details.

4. Run tests and verify everything works:
```bash
elm-test
```

5. Publish to the Elm package repository:
```bash
elm publish
```

For more information, see the [Elm Package Documentation](https://package.elm-lang.org/help/design-guidelines).

## License

[MIT](LICENSE)

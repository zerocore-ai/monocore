# Microsandbox Go SDK

A minimal Go SDK for the Microsandbox project.

## Installation

```bash
go get github.com/yourusername/monocore/sdk/go
```

## Usage

```go
package main

import (
	"fmt"

	microsandbox "github.com/yourusername/monocore/sdk/go"
)

func main() {
	// Print a greeting
	fmt.Println(microsandbox.Greet("World"))
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/go

# Download dependencies
go mod download
```

### Running Tests

```bash
go test ./...
```

### Publishing to Go Modules

Go modules are automatically published when they are pushed to a public repository with proper versioning:

1. Tag your release:

```bash
git tag sdk/go/v0.0.1
git push origin sdk/go/v0.0.1
```

2. Users can import your module using:

```go
import "github.com/yourusername/monocore/sdk/go"
```

Make sure your module follows the [Go Module naming conventions](https://go.dev/doc/modules/managing-dependencies#naming_module).

## License

[MIT](LICENSE)

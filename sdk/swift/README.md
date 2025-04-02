# Microsandbox Swift SDK

A minimal Swift SDK for the Microsandbox project.

## Installation

### Swift Package Manager

Add the following to your `Package.swift` file:

```swift
dependencies: [
    .package(url: "https://github.com/yourusername/monocore.git", from: "0.0.1")
]
```

Then specify the "Microsandbox" product as a dependency for your target:

```swift
targets: [
    .target(
        name: "YourTarget",
        dependencies: [
            .product(name: "Microsandbox", package: "monocore")
        ]
    )
]
```

## Usage

```swift
import Microsandbox

// Print a greeting
Microsandbox.greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/swift

# Build the package
swift build
```

### Running Tests

```bash
swift test
```

### Publishing with Swift Package Manager

Swift Package Manager uses Git tags to identify package versions:

1. Tag your package with a semantic version:

```bash
git tag sdk/swift/0.0.1
git push origin sdk/swift/0.0.1
```

2. Users can then add your package as a dependency in their `Package.swift` file.

For more details, refer to [Swift Package Manager documentation](https://swift.org/package-manager/).

## License

[MIT](LICENSE)

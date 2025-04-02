# Microsandbox Dart SDK

A minimal Dart SDK for the Microsandbox project.

## Installation

```yaml
dependencies:
  microsandbox: ^0.0.1
```

## Usage

```dart
import 'package:microsandbox/microsandbox.dart';

void main() {
  // Print a greeting
  greet('World');
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/dart

# Get dependencies
dart pub get
```

### Running Tests

```bash
dart test
```

### Publishing to pub.dev

1. Create an account on [pub.dev](https://pub.dev/) if you don't have one.

2. Verify your package:

```bash
dart pub publish --dry-run
```

3. Publish your package:

```bash
dart pub publish
```

For more details, refer to [Publishing packages](https://dart.dev/tools/pub/publishing).

## License

[MIT](LICENSE)

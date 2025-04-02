# Microsandbox Objective-C SDK

A minimal Objective-C SDK for the Microsandbox project.

## Installation

### CocoaPods

Add this to your `Podfile`:

```ruby
pod 'Microsandbox', '~> 0.0.1'
```

Then run:

```bash
pod install
```

### Carthage

Add this to your `Cartfile`:

```
github "yourusername/monocore" == 0.0.1
```

Then run:

```bash
carthage update
```

### Manual Installation

1. Clone this repository:

```bash
git clone https://github.com/yourusername/monocore.git
```

2. Drag the `sdk/objc/Microsandbox` folder into your Xcode project.

## Usage

```objc
#import "MSBGreeter.h"

// Print a greeting
NSString *message = [MSBGreeter greet:@"World"];
NSLog(@"%@", message);
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/objc

# Open the project in Xcode
open Microsandbox.xcodeproj
```

### Running Tests

Run tests from Xcode (⌘+U) or use xcodebuild:

```bash
xcodebuild test -project Microsandbox.xcodeproj -scheme Microsandbox -destination 'platform=iOS Simulator,name=iPhone 14'
```

### Publishing to CocoaPods

1. Create an account on [CocoaPods](https://cocoapods.org/) if you don't have one.

2. Create a `Microsandbox.podspec` file (already included in the repository).

3. Validate your pod:

```bash
pod lib lint
```

4. Push your pod to the CocoaPods trunk:

```bash
pod trunk push Microsandbox.podspec
```

For more details, refer to [CocoaPods Guides](https://guides.cocoapods.org/making/making-a-cocoapod.html).

## License

[MIT](LICENSE)

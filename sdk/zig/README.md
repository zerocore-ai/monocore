# Microsandbox Zig SDK

A minimal Zig SDK for the Microsandbox project.

## Installation

Add the package as a dependency in your `build.zig.zon` file:

```zig
.{
    .name = "your-project-name",
    .version = "0.1.0",
    .dependencies = .{
        .microsandbox = .{
            .url = "https://github.com/yourusername/monocore/archive/refs/tags/zig-v0.0.1.tar.gz",
            .hash = "12345", // Replace with the actual hash
        },
    },
}
```

Then in your `build.zig` file, add:

```zig
const microsandbox_dep = b.dependency("microsandbox", .{
    .target = target,
    .optimize = optimize,
});
exe.addModule("microsandbox", microsandbox_dep.module("microsandbox"));
```

## Usage

```zig
const std = @import("std");
const microsandbox = @import("microsandbox");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();
    defer _ = gpa.deinit();

    const message = try microsandbox.greet(allocator, "World");
    defer allocator.free(message);
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/zig
```

### Building and Testing

```bash
zig build
zig build test
```

### Publishing to a Zig Package Registry

Zig doesn't have an official package registry like npm or PyPI. Most Zig packages are distributed via:

1. GitHub releases
2. Git repository references in `build.zig.zon`

To publish your package:

1. Create a GitHub release with a tag (e.g., `zig-v0.0.1`)
2. Users can then add your package as a dependency as shown in the installation section

#### Creating a GitHub Release

```bash
# Tag your release
git tag zig-v0.0.1
git push origin zig-v0.0.1

# Or create a release through the GitHub web interface
```

## License

[MIT](LICENSE)

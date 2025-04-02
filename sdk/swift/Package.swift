// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "Microsandbox",
    products: [
        .library(
            name: "Microsandbox",
            targets: ["Microsandbox"]),
    ],
    dependencies: [
        // No external dependencies
    ],
    targets: [
        .target(
            name: "Microsandbox",
            dependencies: []),
        .testTarget(
            name: "MicrosandboxTests",
            dependencies: ["Microsandbox"]),
    ]
)

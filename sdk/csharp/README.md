# Microsandbox C# SDK

A minimal C# SDK for the Microsandbox project.

## Installation

```bash
dotnet add package Microsandbox
```

Or via the NuGet Package Manager:

```bash
Install-Package Microsandbox
```

## Usage

```csharp
using Microsandbox;

// Print a greeting
HelloWorld.Greet("World");
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/csharp

# Restore dependencies
dotnet restore
```

### Running Tests

```bash
dotnet test
```

### Building the Package

```bash
dotnet build --configuration Release
```

### Publishing to NuGet

1. Create an API key on [NuGet](https://www.nuget.org/)

2. Create a NuGet package:

```bash
dotnet pack --configuration Release
```

3. Publish to NuGet:

```bash
dotnet nuget push bin/Release/Microsandbox.0.0.1.nupkg --api-key YOUR_API_KEY --source https://api.nuget.org/v3/index.json
```

Make sure you have registered for an account on [NuGet.org](https://www.nuget.org/) and have created an API key.

## License

[MIT](LICENSE)

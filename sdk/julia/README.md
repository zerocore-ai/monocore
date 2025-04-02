# Microsandbox.jl

A minimal Julia SDK for the Microsandbox project.

## Installation

```julia
] add Microsandbox
```

Or directly:

```julia
using Pkg
Pkg.add("Microsandbox")
```

## Usage

```julia
using Microsandbox

# Print a greeting
greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/julia

# Start Julia
julia --project=.

# Press ']' to enter Pkg mode
# Activate and install dependencies
activate .
instantiate
```

### Running Tests

```julia
# In Pkg mode (press ']' from the Julia REPL)
test Microsandbox
```

### Building Documentation

```julia
using Pkg
Pkg.add("Documenter")
using Documenter, Microsandbox
makedocs(modules=[Microsandbox])
```

### Publishing to Julia Registry

1. Create a GitHub repository for your package.

2. Tag a release:

```bash
git tag v0.0.1
git push origin v0.0.1
```

3. Register the package with the Julia Registry:

```julia
# In Pkg mode
registry add https://github.com/JuliaRegistries/General
register Microsandbox
```

You can also use [JuliaRegistrator](https://github.com/JuliaRegistries/Registrator.jl) for easier registration.

## License

[MIT](LICENSE)

# Microsandbox OCaml SDK

A minimal OCaml SDK for the Microsandbox project.

## Installation

Using opam:

```bash
opam install microsandbox
```

Or add to your `dune-project` file as a dependency:

```
(depends
 (microsandbox (>= 0.0.1)))
```

## Usage

```ocaml
(* Import the module *)
open Microsandbox

(* Print a greeting *)
let message = greet "World"
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/ocaml

# Create a local opam switch (optional)
opam switch create . --deps-only --with-test
```

### Building

```bash
dune build
```

### Testing

```bash
dune test
```

### Publishing to OPAM Repository

[OPAM](https://opam.ocaml.org/) is the OCaml Package Manager and the standard way to distribute OCaml packages.

To publish your package:

1. Make sure your package builds correctly and passes all tests
2. Generate the `.opam` file using dune (if you're using the `generate_opam_files` option)

```bash
dune build
```

3. Create a repository on GitHub for your package (if not part of a larger repo)
4. Tag a release

```bash
git tag v0.0.1
git push origin v0.0.1
```

5. Submit your package to the OPAM repository:

First, create a PR to the [opam-repository](https://github.com/ocaml/opam-repository):

```bash
# Fork the opam-repository on GitHub

# Clone your fork
git clone https://github.com/yourusername/opam-repository.git
cd opam-repository

# Create a new branch
git checkout -b microsandbox-0.0.1

# Create the package directories and files
mkdir -p packages/microsandbox/microsandbox.0.0.1
cp /path/to/monocore/sdk/ocaml/microsandbox.opam packages/microsandbox/microsandbox.0.0.1/opam

# Add the URL file
cat > packages/microsandbox/microsandbox.0.0.1/url << EOF
url {
  src: "https://github.com/yourusername/monocore/archive/v0.0.1.tar.gz"
  checksum: "md5=<md5sum-of-your-archive>"
}
EOF

# Commit changes
git add .
git commit -m "Add microsandbox.0.0.1"

# Push to your fork
git push origin microsandbox-0.0.1
```

6. Create a pull request from your fork to the main opam-repository

After your PR is merged, users can install your package using `opam install microsandbox`.

## License

[MIT](LICENSE)

# Microsandbox C SDK

A minimal C SDK for the Microsandbox project.

## Installation

Since there's no standard package manager for C libraries, you can install the SDK by:

### Using as a static library

1. Clone this repository:

```bash
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/c
```

2. Build the static library:

```bash
make lib
```

3. Link against the library in your project:

```bash
gcc -o your_program your_program.c -I/path/to/monocore/sdk/c/include -L/path/to/monocore/sdk/c/lib -lmicrosandbox
```

### Using the source directly

Copy the `microsandbox.h` and `microsandbox.c` files directly into your project.

## Usage

```c
#include "microsandbox.h"

int main() {
    // Print a greeting
    char* message = microsandbox_greet("World");
    printf("%s\n", message);
    free(message); // Don't forget to free the allocated memory
    return 0;
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/c
```

### Building

```bash
make
```

### Running Tests

```bash
make test
```

### Distribution

To distribute your C library:

1. Package your header files and static library as a tarball:

```bash
make dist
```

2. Document how to include your library in other projects (as shown in the Installation section).

## License

[MIT](LICENSE)

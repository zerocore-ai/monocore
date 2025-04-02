# Microsandbox C++ SDK

A minimal C++ SDK for the Microsandbox project.

## Installation

### Using CMake (recommended)

1. Clone this repository:

```bash
git clone https://github.com/yourusername/monocore.git
```

2. Add the SDK to your CMake project:

```cmake
# In your CMakeLists.txt
add_subdirectory(/path/to/monocore/sdk/cpp)
target_link_libraries(your_target microsandbox)
```

### Using vcpkg

```bash
vcpkg install microsandbox
```

### Using Conan

```bash
conan install microsandbox/0.0.1
```

## Usage

```cpp
#include <iostream>
#include <microsandbox/microsandbox.hpp>

int main() {
    // Print a greeting
    std::string message = microsandbox::greet("World");
    std::cout << message << std::endl;
    return 0;
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/cpp

# Create a build directory
mkdir build && cd build

# Configure and build
cmake ..
make
```

### Running Tests

```bash
# In the build directory
ctest
```

### Publishing to Package Managers

#### vcpkg

1. Fork the [vcpkg repository](https://github.com/microsoft/vcpkg)
2. Add a new port in `ports/microsandbox/`
3. Create a PR against the vcpkg repository

#### Conan

1. Create a Conan recipe:

```bash
conan new microsandbox/0.0.1 -t
```

2. Test and upload to Conan Center:

```bash
conan create . microsandbox/0.0.1@user/testing
conan upload microsandbox/0.0.1@user/testing -r conan-center
```

For more information, see the [Conan documentation](https://docs.conan.io/en/latest/uploading_packages/using_artifactory.html).

## License

[MIT](LICENSE)

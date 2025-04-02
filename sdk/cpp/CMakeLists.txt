cmake_minimum_required(VERSION 3.10)
project(microsandbox VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Add include directories
include_directories(include)

# Add library target
add_library(microsandbox STATIC
    src/microsandbox.cpp
)

# Generate export header
include(GenerateExportHeader)
generate_export_header(microsandbox)

# Set public headers
set_target_properties(microsandbox PROPERTIES PUBLIC_HEADER include/microsandbox/microsandbox.hpp)

# Add test executable
add_executable(test_microsandbox test/test_microsandbox.cpp)
target_link_libraries(test_microsandbox microsandbox)

# Add test to CTest
enable_testing()
add_test(NAME test_microsandbox COMMAND test_microsandbox)

# Install targets
install(TARGETS microsandbox
    EXPORT microsandboxTargets
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
    PUBLIC_HEADER DESTINATION include/microsandbox
)

# Export targets
install(EXPORT microsandboxTargets
    FILE microsandboxTargets.cmake
    NAMESPACE microsandbox::
    DESTINATION lib/cmake/microsandbox
)

# Create and install package configuration
include(CMakePackageConfigHelpers)
configure_package_config_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/cmake/microsandboxConfig.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/microsandboxConfig.cmake
    INSTALL_DESTINATION lib/cmake/microsandbox
)

write_basic_package_version_file(
    ${CMAKE_CURRENT_BINARY_DIR}/microsandboxConfigVersion.cmake
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY SameMajorVersion
)

install(FILES
    ${CMAKE_CURRENT_BINARY_DIR}/microsandboxConfig.cmake
    ${CMAKE_CURRENT_BINARY_DIR}/microsandboxConfigVersion.cmake
    DESTINATION lib/cmake/microsandbox
)

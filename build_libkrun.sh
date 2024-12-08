#!/bin/sh

# build_libkrun.sh
# ---------------
# This script automates the building and installation of libkrun and libkrunfw libraries,
# which are essential components for running micro virtual machines.
#
# Usage:
#   ./build_libkrun.sh [options]
#
# Options:
#   --no-cleanup    Skip cleanup of build directories and VMs after completion
#   --force-build   Force rebuild even if libraries are already installed
#
# Requirements:
#   - git
#   - make
#   - Rust/Cargo (for libkrun)
#   - Python (for libkrunfw)
#   - On macOS: krunvm must be installed (brew tap slp/krun && brew install krunvm)
#
# The script performs the following tasks:
#   1. Creates build directory if needed
#   2. Clones libkrunfw from Github
#   3. Clones libkrun from GitHub
#   4. Builds and installs both libraries
#   5. Creates non-versioned variants of libraries (needed for CI)
#   6. Handles cleanup unless --no-cleanup is specified
#
# Library Installation Paths:
#   Linux:
#     - /usr/local/lib64/libkrun.so
#     - /usr/local/lib64/libkrunfw.so
#   macOS:
#     - /usr/local/lib/libkrun.dylib
#     - /usr/local/lib/libkrunfw.dylib
#
# Error Handling:
#   - The script checks for errors after each critical operation
#   - Exits with status code 1 on any failure
#   - Performs cleanup on exit unless --no-cleanup is specified
#
# Platform Support:
#   - Linux: Full support
#   - macOS: Requires krunvm, handles platform-specific paths and library extensions
#   - Other platforms are not supported
#
# Examples:
#   # Standard build and install
#   ./build_libkrun.sh
#
#   # Build without cleaning up build directory
#   ./build_libkrun.sh --no-cleanup
#
#   # Force rebuild even if libraries exist
#   ./build_libkrun.sh --force-build
#
#   # Combine options
#   ./build_libkrun.sh --no-cleanup --force-build

# Color variables
RED="\033[1;31m"
GREEN="\033[1;32m"
YELLOW="\033[1;33m"
RESET="\033[0m"

# Logging functions
info() {
    printf "${GREEN}:: %s${RESET}\n" "$1"
}

warn() {
    printf "${YELLOW}:: %s${RESET}\n" "$1"
}

error() {
    printf "${RED}:: %s${RESET}\n" "$1"
}

# Store the original working directory
ORIGINAL_DIR="$(pwd)"

# Set up variables
BUILD_DIR="$ORIGINAL_DIR/build"
LIBKRUNFW_REPO="https://github.com/appcypher/libkrunfw.git"
LIBKRUN_REPO="https://github.com/appcypher/libkrun.git"
NO_CLEANUP=false
FORCE_BUILD=false

# Parse command line arguments
for arg in "$@"
do
    case $arg in
      --no-clean|--no-cleanup)
        NO_CLEANUP=true
        shift
        ;;
      --force|--force-build)
        FORCE_BUILD=true
        shift
        ;;
    esac
done

# Determine the OS type
OS_TYPE="$(uname -s)"

# Check if krunvm is installed on macOS, if applicable
if [ "$OS_TYPE" = "Darwin" ]; then
  if ! command -v krunvm &> /dev/null; then
    printf "${RED}krunvm command not found. Please install it using: brew tap slp/krun && brew install krunvm${RESET}\n"
    exit 1
  fi
fi

# After OS detection...
if [ "$OS_TYPE" = "Linux" ]; then
    if ! command -v patchelf &> /dev/null; then
        error "patchelf command not found. Please install it using your package manager."
        exit 1
    fi
fi

# Function to handle cleanup
cleanup() {
  if [ "$NO_CLEANUP" = true ]; then
    info "Skipping cleanup as requested."
    return
  fi

  warn "Cleaning up..."

  cd "$ORIGINAL_DIR" || { error "Failed to change back to original directory"; exit 1; }

  rm -rf "$BUILD_DIR"
  if [ "$OS_TYPE" = "Darwin" ]; then
    warn "Deleting libkrunfw-builder VM..."
    krunvm delete libkrunfw-builder

    warn "Deleting libkrun-builder VM..."
    krunvm delete libkrun-builder
  fi
  info "Cleanup complete."
}

# Trap EXIT signal to run cleanup
trap cleanup EXIT

# Function to check command success
check_success() {
  if [ $? -ne 0 ]; then
    error "Error occurred: $1"
    exit 1
  fi
}

# Common function to check for existing installations
check_existing_lib() {
    if [ "$FORCE_BUILD" = true ]; then
        info "Force build enabled. Skipping check for existing $1."
        return 0
    fi

    local lib_name="$1"

    # Get ABI version from the appropriate Makefile
    local abi_version=$(get_abi_version "$BUILD_DIR/$lib_name/Makefile")

    case "$OS_TYPE" in
        Linux)
            lib_path="$BUILD_DIR/$lib_name.so.$abi_version"
            ;;
        Darwin)
            lib_path="$BUILD_DIR/$lib_name.$abi_version.dylib"
            ;;
        *)
            error "Unsupported OS: $OS_TYPE"
            exit 1
            ;;
    esac

    if [ -f "$lib_path" ]; then
        info "$lib_name already exists in $lib_path. Skipping build."
        return 1
    fi
    return 0
}

# Function to create build directory
create_build_directory() {
  cd "$ORIGINAL_DIR" || { error "Failed to change to original directory"; exit 1; }

  if [ -d "$BUILD_DIR" ]; then
    info "Build directory already exists. Skipping creation..."
  else
    info "Creating build directory..."
    mkdir -p "$BUILD_DIR"
    check_success "Failed to create build directory"
  fi
}

# Common function to clone repositories
clone_repo() {
  cd "$BUILD_DIR" || { error "Failed to change to build directory"; exit 1; }

  local repo_url="$1"
  local repo_name="$2"

  if [ -d "$repo_name" ]; then
    info "$repo_name directory already exists. Skipping cloning..."
  else
    info "Cloning $repo_name repository..."
    git clone "$repo_url"
    check_success "Failed to clone $repo_name repository"
  fi
}

# Function to extract ABI version from Makefile
get_abi_version() {
    local makefile="$1"
    local abi_version=$(grep "^ABI_VERSION.*=" "$makefile" | cut -d'=' -f2 | tr -d ' ')
    if [ -z "$abi_version" ]; then
        error "Could not determine ABI version from $makefile"
        exit 1
    fi
    echo "$abi_version"
}

# Function to extract FULL_VERSION from Makefile
get_full_version() {
    local makefile="$1"
    local full_version=$(grep "^FULL_VERSION.*=" "$makefile" | cut -d'=' -f2 | tr -d ' ')
    if [ -z "$full_version" ]; then
        error "Could not determine FULL_VERSION from $makefile"
        exit 1
    fi
    echo "$full_version"
}

# Function to build and copy libkrunfw
build_libkrunfw() {
    cd "$BUILD_DIR/libkrunfw" || { error "Failed to change to libkrunfw directory"; exit 1; }

    local abi_version=$(get_abi_version "Makefile")
    info "Detected libkrunfw ABI version: $abi_version"

    info "Building libkrunfw..."
    export PYTHONPATH="$HOME/.local/lib/python3.*/site-packages:$PYTHONPATH"

    case "$OS_TYPE" in
        Darwin)
            # On macOS, we need sudo to allow krunvm set xattr on the volume
            sudo make PYTHONPATH="$PYTHONPATH"
            ;;
        *)
            make PYTHONPATH="$PYTHONPATH"
            ;;
    esac
    check_success "Failed to build libkrunfw"

    # Copy the library to build directory and create symlink
    info "Copying libkrunfw to build directory..."
    cd "$BUILD_DIR" || { error "Failed to change to build directory"; exit 1; }
    case "$OS_TYPE" in
        Linux)
            cp libkrunfw/libkrunfw.so.$abi_version.* "libkrunfw.so.$abi_version"
            patchelf --set-rpath '$ORIGIN' "libkrunfw.so.$abi_version"
            ln -sf "libkrunfw.so.$abi_version" "libkrunfw.so"
            ;;
        Darwin)
            cp libkrunfw/libkrunfw.$abi_version.dylib "libkrunfw.$abi_version.dylib"
            install_name_tool -id "@rpath/libkrunfw.$abi_version.dylib" "libkrunfw.$abi_version.dylib"
            ln -sf "libkrunfw.$abi_version.dylib" "libkrunfw.dylib"
            ;;
        *)
            error "Unsupported OS: $OS_TYPE"
            exit 1
            ;;
    esac
    check_success "Failed to copy libkrunfw"
}

# Function to build and copy libkrun
build_libkrun() {
    cd "$BUILD_DIR/libkrun" || { error "Failed to change to libkrun directory"; exit 1; }

    local abi_version=$(get_abi_version "Makefile")
    local full_version=$(get_full_version "Makefile")
    info "Detected libkrun ABI version: $abi_version"
    info "Detected libkrun FULL version: $full_version"

    info "Building libkrun..."
    # Update library path to use our build directory
    export LIBRARY_PATH="$BUILD_DIR:$LIBRARY_PATH"
    export PATH="$HOME/.cargo/bin:$PATH"

    make LIBRARY_PATH="$LIBRARY_PATH" PATH="$PATH"
    check_success "Failed to build libkrun"

    # Copy and rename the library to build directory and create symlink
    info "Copying libkrun to build directory..."
    cd "$BUILD_DIR" || { error "Failed to change to build directory"; exit 1; }
    case "$OS_TYPE" in
        Linux)
            cp libkrun/target/release/libkrun.so.$full_version "libkrun.so.$abi_version"
            patchelf --set-rpath '$ORIGIN' "libkrun.so.$abi_version"
            patchelf --set-needed "libkrunfw.so.4" "libkrun.so.$abi_version"
            ln -sf "libkrun.so.$abi_version" "libkrun.so"
            ;;
        Darwin)
            cp libkrun/target/release/libkrun.$full_version.dylib "libkrun.$abi_version.dylib"
            install_name_tool -id "@rpath/libkrun.$abi_version.dylib" "libkrun.$abi_version.dylib"
            install_name_tool -change "libkrunfw.4.dylib" "@rpath/libkrunfw.4.dylib" "libkrun.$abi_version.dylib"
            ln -sf "libkrun.$abi_version.dylib" "libkrun.dylib"
            ;;
        *)
            error "Unsupported OS: $OS_TYPE"
            exit 1
            ;;
    esac
    check_success "Failed to copy libkrun"
}

# Main script execution
check_existing_lib "libkrunfw"
if [ $? -eq 0 ]; then
    create_build_directory
    clone_repo "$LIBKRUNFW_REPO" "libkrunfw"
    build_libkrunfw
fi

check_existing_lib "libkrun"
if [ $? -eq 0 ]; then
    create_build_directory
    clone_repo "$LIBKRUN_REPO" "libkrun"
    build_libkrun
fi

# Finished
info "Setup complete."

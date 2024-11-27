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
#   - sudo privileges
#   - git
#   - make
#   - Rust/Cargo (for libkrun)
#   - Python (for libkrunfw)
#   - On macOS: krunvm must be installed (brew install krunvm)
#
# The script performs the following tasks:
#   1. Checks for sudo privileges and maintains sudo session
#   2. Creates build directory if needed
#   3. Clones libkrunfw from Github
#   4. Clones libkrun from GitHub
#   5. Builds and installs both libraries
#   6. Creates non-versioned variants of libraries (needed for CI)
#   7. Handles cleanup unless --no-cleanup is specified
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

# Elevate privileges at the start to avoid repeated prompts
if ! sudo -v; then
    error "This script requires sudo privileges. Please run with sudo or grant sudo access."
    exit 1
fi

# Keep sudo alive in the background
while true; do
    sudo -n true
    sleep 60
    kill -0 "$$" || exit
done 2>/dev/null &

# Store the original working directory
ORIGINAL_DIR="$(pwd)"

# Set up variables
BUILD_DIR="$ORIGINAL_DIR/build"
LIBKRUNFW_REPO="https://github.com/appcypher/libkrunfw.git"
LIBKRUN_REPO="https://github.com/appcypher/libkrun.git"
LIB_DIR="/usr/local/lib"
LIB64_DIR="/usr/local/lib64"
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
    printf "${RED}krunvm command not found. Please install it using: brew install krunvm${RESET}\n"
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

  sudo rm -rf "$BUILD_DIR"
  if [ "$OS_TYPE" = "Darwin" ]; then
    warn "Deleting libkrunfw-builder VM..."
    sudo krunvm delete libkrunfw-builder

    warn "Deleting libkrun-builder VM..."
    sudo krunvm delete libkrun-builder
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
    if [ "$FORCE_BUILD" = true ]; then  # Check if force build is enabled
        info "Force build enabled. Skipping check for existing $1."
        return 0
    fi

    local lib_name="$1"
    case "$OS_TYPE" in
        Linux)
          lib_path="$LIB64_DIR/$lib_name.so"
          ;;
        Darwin)
          lib_path="$LIB_DIR/$lib_name.dylib"
          ;;
        *)
          error "Unsupported OS: $OS_TYPE"
          exit 1
          ;;
    esac

    if [ -f "$lib_path" ]; then
        info "$lib_name already exists in $lib_path. Skipping cloning, building, and installation."
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

# Function to create non-versioned library
create_non_versioned_lib() {
  local lib_name="$1"
  local extension="$2"

  local versioned_lib=$(ls "${lib_name}"*".${extension}"*"" 2>/dev/null | head -n 1)

  if [ -n "$versioned_lib" ]; then
    sudo cp "$versioned_lib" "${lib_name}.${extension}"
    check_success "Failed to create non-versioned ${lib_name}.${extension}"
    info "Created non-versioned ${lib_name}.${extension}"
  else
    error "No ${lib_name}.*.${extension} file found"
    exit 1
  fi
}

# Function to build and install a library
build_and_install_lib() {
  local lib_name="$1"

  cd "$BUILD_DIR/$lib_name" || { error "Failed to change to $lib_name directory"; exit 1; }

  # Build the library
  info "Building $lib_name..."
  if [ "$lib_name" = "libkrunfw" ]; then
    # Set PYTHONPATH to include the user's site-packages
    export PYTHONPATH="$HOME/.local/lib/python3.*/site-packages:$PYTHONPATH"

    # Use sudo -E to preserve the PYTHONPATH
    sudo -E make PYTHONPATH="$PYTHONPATH"
  else
    # For libkrun
    info "Setting LIBRARY_PATH for libkrunfw..."
    export LIBRARY_PATH="$LIB64_DIR:$LIB_DIR:$LIBRARY_PATH"

    # Ensure Rust and Cargo are in the PATH
    export PATH="$HOME/.cargo/bin:$PATH"

    # Use sudo -E to preserve the PATH and LIBRARY_PATH
    sudo -E make LIBRARY_PATH="$LIBRARY_PATH" PATH="$PATH"
  fi
  check_success "Failed to build $lib_name"

  # Install the library
  info "Installing $lib_name..."
  sudo make install
  check_success "Failed to install $lib_name"

  # On macOS, patch the dylib install name to point to its actual location
  if [ "$OS_TYPE" = "Darwin" ]; then
    info "Patching dylib install name for $lib_name..."
    sudo install_name_tool -id "$LIB_DIR/${lib_name}.dylib" "$LIB_DIR/${lib_name}.dylib"
    check_success "Failed to patch dylib install name for $lib_name"
  fi

  # Create non-versioned variant of libkrunfw.
  # Needed for GH action CI builds.
  if [ "$lib_name" = "libkrunfw" ]; then
    info "Creating non-versioned variant of $lib_name..."
    case "$OS_TYPE" in
      Linux)
        create_non_versioned_lib "libkrunfw" "so"
        ;;
      Darwin)
        create_non_versioned_lib "libkrunfw" "dylib"
        ;;
      *)
        error "Unsupported OS: $OS_TYPE"
        exit 1
        ;;
    esac
  fi
}

# Main script execution
check_existing_lib "libkrunfw"
if [ $? -eq 0 ]; then
    create_build_directory
    clone_repo "$LIBKRUNFW_REPO" "libkrunfw"
    build_and_install_lib "libkrunfw"
fi

check_existing_lib "libkrun"
if [ $? -eq 0 ]; then
    create_build_directory
    clone_repo "$LIBKRUN_REPO" "libkrun"
    build_and_install_lib "libkrun"
fi

# Finished
info "Setup complete."

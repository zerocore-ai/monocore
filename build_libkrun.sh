#!/bin/sh

# Elevate privileges at the start to avoid repeated prompts
sudo -v

# Set up variables
BUILD_DIR="build"
LIBKRUNFW_REPO="https://github.com/containers/libkrunfw.git"
LIBKRUN_REPO="https://github.com/appcypher/libkrun.git"
LIB_DIR="/usr/local/lib"
LIB64_DIR="/usr/local/lib64"

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

# Determine the OS type
OS_TYPE="$(uname -s)"

# Store the original working directory
ORIGINAL_DIR="$(pwd)"

# Check if krunvm is installed on macOS, if applicable
if [ "$OS_TYPE" = "Darwin" ]; then
  if ! command -v krunvm &> /dev/null; then
    printf "${RED}krunvm command not found. Please install it using: brew install krunvm${RESET}\n"
    exit 1
  fi
fi

# Function to handle cleanup
cleanup() {
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
  if [ -d "$BUILD_DIR" ]; then
    info "Build directory already exists. Skipping creation..."
  else
    info "Creating build directory..."
    mkdir -p "$BUILD_DIR"
    check_success "Failed to create build directory"
  fi
  cd "$BUILD_DIR" || { error "Failed to change to build directory"; exit 1; }
}

# Common function to clone repositories
clone_repo() {
  local repo_url="$1"
  local repo_name="$2"
  info "Cloning $repo_name repository..."
  git clone "$repo_url"
  check_success "Failed to clone $repo_name repository"
}

# Function to build and install a library
build_and_install_lib() {
  local lib_name="$1"
  cd "$lib_name" || { error "Failed to change to $lib_name directory"; exit 1; }

  if [ "$lib_name" = "libkrun" ]; then
    info "Setting LIBRARY_PATH for libkrunfw..."
    export LIBRARY_PATH="$LIB64_DIR:$LIB_DIR:$LIBRARY_PATH"
  fi

  info "Building $lib_name..."
  sudo make
  check_success "Failed to build $lib_name"

  info "Installing $lib_name..."
  sudo make install
  check_success "Failed to install $lib_name"

  cd ..
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

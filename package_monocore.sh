#!/bin/sh

# package_monocore.sh
# ------------------
# This script packages monocore and its dependencies into a distributable tarball.
#
# Usage:
#   ./package_monocore.sh <semver>
#
# Arguments:
#   semver    Semantic version for the package (required, e.g., 0.1.0)
#
# The script performs the following tasks:
#   1. Builds monocore and its dependencies
#   2. Creates a versioned directory with OS and architecture info
#   3. Copies all necessary binaries and libraries
#   4. Creates a tarball and its SHA256 checksum
#
# Example:
#   ./package_monocore.sh 0.1.0

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
    exit 1
}

# Check for required semver argument
if [ $# -ne 1 ]; then
    error "Usage: $0 <semver>"
fi

SEMVER="$1"
# Validate semver format (basic check)
if ! echo "$SEMVER" | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' >/dev/null; then
    error "Invalid semver format. Expected format: X.Y.Z (e.g., 0.1.0)"
fi

# Determine OS and architecture
OS_TYPE="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)
        ARCH="x86_64"
        ;;
    aarch64)
        ARCH="aarch64"
        ;;
    arm64)
        ARCH="arm64"
        ;;
esac

# Set up variables
ORIGINAL_DIR="$(pwd)"
BUILD_DIR="$ORIGINAL_DIR/build"
PACKAGE_NAME="monocore-${SEMVER}-${OS_TYPE}-${ARCH}"
PACKAGE_DIR="$BUILD_DIR/$PACKAGE_NAME"

# Function to check command success
check_success() {
    if [ $? -ne 0 ]; then
        error "Error occurred: $1"
    fi
}

# Build monocore and dependencies
info "Building monocore..."
make build
check_success "Failed to build monocore"

# Create package directory
info "Creating package directory..."
mkdir -p "$PACKAGE_DIR"
check_success "Failed to create package directory"

info "Copying binaries..."
# Copy monocore and monokrun
cp "$BUILD_DIR/monocore" "$PACKAGE_DIR/"
check_success "Failed to copy monocore binary"
cp "$BUILD_DIR/monokrun" "$PACKAGE_DIR/"
check_success "Failed to copy monokrun binary"

# Copy libraries based on OS type
info "Copying libraries..."
if [ "$OS_TYPE" = "darwin" ]; then
    # Find and copy libkrun
    LIBKRUN=$(find "$BUILD_DIR" -maxdepth 1 -name "libkrun.*.dylib" | head -n 1)
    cp "$LIBKRUN" "$PACKAGE_DIR/$(basename "$LIBKRUN")"
    check_success "Failed to copy libkrun"

    # Find and copy libkrunfw
    LIBKRUNFW=$(find "$BUILD_DIR" -maxdepth 1 -name "libkrunfw.*.dylib" | head -n 1)
    cp "$LIBKRUNFW" "$PACKAGE_DIR/$(basename "$LIBKRUNFW")"
    check_success "Failed to copy libkrunfw"
else
    # Find and copy libkrun
    LIBKRUN=$(find "$BUILD_DIR" -maxdepth 1 -name "libkrun.so.*" | head -n 1)
    cp "$LIBKRUN" "$PACKAGE_DIR/$(basename "$LIBKRUN")"
    check_success "Failed to copy libkrun"

    # Find and copy libkrunfw
    LIBKRUNFW=$(find "$BUILD_DIR" -maxdepth 1 -name "libkrunfw.so.*" | head -n 1)
    cp "$LIBKRUNFW" "$PACKAGE_DIR/$(basename "$LIBKRUNFW")"
    check_success "Failed to copy libkrunfw"
fi

info "Creating tarball..."
tar -czvf "$BUILD_DIR/$PACKAGE_NAME.tar.gz" -C "$BUILD_DIR" "$PACKAGE_NAME"
check_success "Failed to create tarball"

info "Generating SHA256 checksum..."
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$BUILD_DIR/$PACKAGE_NAME.tar.gz" > "$BUILD_DIR/$PACKAGE_NAME.tar.gz.sha256"
    check_success "Failed to generate SHA256 checksum"

    info "Verifying checksum..."
    sha256sum -c "$BUILD_DIR/$PACKAGE_NAME.tar.gz.sha256"
    check_success "Checksum verification failed"
else
    shasum -a 256 "$BUILD_DIR/$PACKAGE_NAME.tar.gz" > "$BUILD_DIR/$PACKAGE_NAME.tar.gz.sha256"
    check_success "Failed to generate SHA256 checksum"

    info "Verifying checksum..."
    shasum -a 256 -c "$BUILD_DIR/$PACKAGE_NAME.tar.gz.sha256"
    check_success "Checksum verification failed"
fi

info "Package created successfully:"
info "  - $BUILD_DIR/$PACKAGE_NAME.tar.gz"
info "  - $BUILD_DIR/$PACKAGE_NAME.tar.gz.sha256"

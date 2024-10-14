#!/bin/sh

set -e

# Create directory from MONOCORE_HOME environment variable, default to $HOME/.monocore
MONOCORE_DIR="${MONOCORE_HOME:-$HOME/.monocore}"
mkdir -p "$MONOCORE_DIR"

# Detect OS and architecture
OS="unknown"
ARCH="unknown"

case "$(uname -s)" in
    Linux*)     OS="Linux";;
    Darwin*)    OS="macOS";;
    *)          OS="unsupported";;
esac

case "$(uname -m)" in
    x86_64)     ARCH="x86_64";;
    arm64)      ARCH="arm64";;
    aarch64)    ARCH="arm64";;
    *)          ARCH="unknown";;
esac

if [ "$OS" = "unsupported" ]; then
    echo "Unsupported operating system. Exiting."
    exit 1
fi

# Write Hello World to hello_world.txt with OS and Arch info
HELLO_FILE="$MONOCORE_DIR/hello_world.txt"
echo "Hello World ${OS} ${ARCH}!" > "$HELLO_FILE"

# Output completion message
echo "Installation complete. File created at $HELLO_FILE"

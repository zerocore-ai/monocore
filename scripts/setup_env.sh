#!/bin/sh

# setup_env.sh
# -----------
# This script configures the user's shell environment to include ~/.local/bin
# and ~/.local/lib in the appropriate path variables.
#
# Usage:
#   ./setup_env.sh [options]
#
# Options:
#   --force    Force update even if paths are already configured
#
# Supported shells:
#   - bash
#   - zsh
#   - fish
#   - sh
#
# The script performs the following tasks:
#   1. Detects user's shell
#   2. Creates ~/.local/bin and ~/.local/lib if they don't exist
#   3. Updates appropriate shell config files
#   4. Sets up PATH and library paths (LD_LIBRARY_PATH/DYLD_LIBRARY_PATH)

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

# Default values
FORCE=false

# Parse command line arguments
for arg in "$@"; do
    case $arg in
        --force)
            FORCE=true
            shift
            ;;
    esac
done

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     LIB_PATH_VAR="LD_LIBRARY_PATH";;
    Darwin*)    LIB_PATH_VAR="DYLD_LIBRARY_PATH";;
    *)          error "Unsupported operating system: $OS"; exit 1;;
esac

# Create directories if they don't exist
create_directories() {
    info "Creating local directories if needed..."
    mkdir -p "$HOME/.local/bin" "$HOME/.local/lib"
    if [ $? -ne 0 ]; then
        error "Failed to create directories"
        exit 1
    fi
}

# Function to check if a line exists in a file
line_exists() {
    grep -Fxq "$1" "$2" 2>/dev/null
}

# Function to add environment config for sh/bash/zsh
setup_posix_shell() {
    local shell_rc="$1"
    local shell_name="$2"

    info "Setting up $shell_name configuration..."

    # Check if config file exists
    if [ ! -f "$shell_rc" ]; then
        info "Creating new config file: $shell_rc"
        mkdir -p "$(dirname "$shell_rc")"
        echo "# Created by monocore setup" > "$shell_rc"
        echo >> "$shell_rc"
    fi

    # PATH configuration
    if ! line_exists 'export PATH="$HOME/.local/bin:$PATH"' "$shell_rc" || [ "$FORCE" = true ]; then
        echo >> "$shell_rc"
        echo '# Added by setup_env.sh' >> "$shell_rc"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$shell_rc"
        info "Added PATH configuration to $shell_rc"
    fi

    # Library path configuration
    if ! line_exists "export $LIB_PATH_VAR=\"\$HOME/.local/lib:\$$LIB_PATH_VAR\"" "$shell_rc" || [ "$FORCE" = true ]; then
        echo "export $LIB_PATH_VAR=\"\$HOME/.local/lib:\$$LIB_PATH_VAR\"" >> "$shell_rc"
        info "Added library path configuration to $shell_rc"
    fi
}

# Function to set up fish shell
setup_fish() {
    local fish_config="$HOME/.config/fish/config.fish"

    info "Setting up fish configuration..."

    # Check if config directory and file exist
    if [ ! -f "$fish_config" ]; then
        info "Creating new fish config file: $fish_config"
        mkdir -p "$(dirname "$fish_config")"
        echo "# Created by monocore setup" > "$fish_config"
        echo >> "$fish_config"
    fi

    # PATH configuration
    if ! line_exists "set -gx PATH $HOME/.local/bin \$PATH" "$fish_config" || [ "$FORCE" = true ]; then
        echo >> "$fish_config"
        echo '# Added by setup_env.sh' >> "$fish_config"
        echo "set -gx PATH $HOME/.local/bin \$PATH" >> "$fish_config"
        info "Added PATH configuration to $fish_config"
    fi

    # Library path configuration
    if ! line_exists "set -gx $LIB_PATH_VAR $HOME/.local/lib \$$LIB_PATH_VAR" "$fish_config" || [ "$FORCE" = true ]; then
        echo "set -gx $LIB_PATH_VAR $HOME/.local/lib \$$LIB_PATH_VAR" >> "$fish_config"
        info "Added library path configuration to $fish_config"
    fi
}

# Add this function to check if a program exists
check_shell() {
    command -v "$1" >/dev/null 2>&1
}

# Main setup function
setup_shell_env() {
    create_directories

    info "Configuring detected shells..."

    # Configure bash if installed
    if check_shell bash; then
        setup_posix_shell "$HOME/.bashrc" "bash"
    fi

    # Configure zsh if installed
    if check_shell zsh; then
        setup_posix_shell "$HOME/.zshrc" "zsh"
    fi

    # Configure fish if installed
    if check_shell fish; then
        setup_fish
    fi

    # Always configure .profile for POSIX shell compatibility
    setup_posix_shell "$HOME/.profile" "sh"

    info "Environment setup complete for detected shells!"
    info "Please restart your shell or source your shell's config file"
}

# Run main setup
setup_shell_env

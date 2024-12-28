# =============================================================================
# Monocore Integration Tests Makefile
# =============================================================================

# -----------------------------------------------------------------------------
# System Detection and Architecture
# -----------------------------------------------------------------------------
OS := $(shell uname -s)
ARCH := $(shell uname -m)
ifeq ($(ARCH),aarch64)
	ARCH := arm64
endif
ifeq ($(ARCH),x86_64)
	ARCH := x86_64
endif

# -----------------------------------------------------------------------------
# Build Paths and Directories
# -----------------------------------------------------------------------------
ROOT_DIR := $(shell git rev-parse --show-toplevel)
BUILD_DIR := $(ROOT_DIR)/build
MONOCORE_BIN := $(BUILD_DIR)/monocore

# -----------------------------------------------------------------------------
# Test Configuration
# -----------------------------------------------------------------------------
CARGO_TEST_FLAGS ?=

# -----------------------------------------------------------------------------
# Phony Targets Declaration
# -----------------------------------------------------------------------------
.PHONY: all test clean

# -----------------------------------------------------------------------------
# Main Targets
# -----------------------------------------------------------------------------
all: test

test: $(MONOCORE_BIN)
	@echo "Running tests..."
	cd .. && MONOCORE_BIN=$(MONOCORE_BIN) cargo test $(CARGO_TEST_FLAGS)

$(MONOCORE_BIN):
	@echo "Building monocore binary..."
	cd $(ROOT_DIR) && make build

clean:
	@echo "Cleaning up..."
	cd $(ROOT_DIR) && make clean

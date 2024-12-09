# System and architecture variables
OS := $(shell uname -s)
ARCH := $(shell uname -m)
ifeq ($(ARCH),aarch64)
	ARCH := arm64
endif
ifeq ($(ARCH),x86_64)
	ARCH := x86_64
endif

# Installation paths
HOME_LIB := $(HOME)/.local/lib
HOME_BIN := $(HOME)/.local/bin

# Binary paths
MONOCORE_RELEASE_BIN := target/release/monocore
MONOKRUN_RELEASE_BIN := target/release/monokrun
BUILD_DIR := build

# Get library paths and versions
ifeq ($(OS),Darwin)
	LIBKRUNFW_FILE := $(shell ls $(BUILD_DIR)/libkrunfw.*.dylib 2>/dev/null | head -n1)
	LIBKRUN_FILE := $(shell ls $(BUILD_DIR)/libkrun.*.dylib 2>/dev/null | head -n1)
else
	LIBKRUNFW_FILE := $(shell ls $(BUILD_DIR)/libkrunfw.so.* 2>/dev/null | head -n1)
	LIBKRUN_FILE := $(shell ls $(BUILD_DIR)/libkrun.so.* 2>/dev/null | head -n1)
endif

# Feature flags
FEATURES ?=
ifdef OVERLAYFS
	FEATURES += --features overlayfs
endif

# Phony targets
.PHONY: all build install clean deps

# Default target
all: build

# Build the release binaries
build: deps $(MONOCORE_RELEASE_BIN) $(MONOKRUN_RELEASE_BIN)
	# Copy binaries to build directory
	@cp $(MONOCORE_RELEASE_BIN) $(BUILD_DIR)/
	@cp $(MONOKRUN_RELEASE_BIN) $(BUILD_DIR)/
	@echo "Build artifacts copied to $(BUILD_DIR)/"

$(MONOCORE_RELEASE_BIN): deps
	@mkdir -p $(BUILD_DIR)
	cd monocore
ifeq ($(OS),Darwin)
	RUSTFLAGS="-C link-args=-Wl,-rpath,@executable_path/../lib" cargo build --release --bin monocore $(FEATURES)
	codesign --entitlements monocore/monocore.entitlements --force -s - $@
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,\$$ORIGIN/../lib" cargo build --release --bin monocore $(FEATURES)
ifdef OVERLAYFS
	sudo setcap cap_sys_admin+ep $@
endif
endif

$(MONOKRUN_RELEASE_BIN): deps
	cd monocore
ifeq ($(OS),Darwin)
	RUSTFLAGS="-C link-args=-Wl,-rpath,@executable_path/../lib" cargo build --release --bin monokrun $(FEATURES)
	codesign --entitlements monocore/monocore.entitlements --force -s - $@
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,\$$ORIGIN/../lib" cargo build --release --bin monokrun $(FEATURES)
ifdef OVERLAYFS
	sudo setcap cap_sys_admin+ep $@
endif
endif

# Install binaries and libraries
install: build
	# Create directories if they don't exist
	install -d $(HOME_BIN)
	install -d $(HOME_LIB)

	# Install binaries
	install -m 755 $(BUILD_DIR)/monocore $(HOME_BIN)/monocore
	install -m 755 $(BUILD_DIR)/monokrun $(HOME_BIN)/monokrun

	# Create mc symlink
	ln -sf $(HOME_BIN)/monocore $(HOME_BIN)/mc

	# Install libraries and create symlinks
	@if [ -n "$(LIBKRUNFW_FILE)" ]; then \
		install -m 755 $(LIBKRUNFW_FILE) $(HOME_LIB)/; \
		cd $(HOME_LIB) && ln -sf $(notdir $(LIBKRUNFW_FILE)) libkrunfw.dylib; \
	else \
		echo "Warning: libkrunfw library not found in build directory"; \
	fi
	@if [ -n "$(LIBKRUN_FILE)" ]; then \
		install -m 755 $(LIBKRUN_FILE) $(HOME_LIB)/; \
		cd $(HOME_LIB) && ln -sf $(notdir $(LIBKRUN_FILE)) libkrun.dylib; \
	else \
		echo "Warning: libkrun library not found in build directory"; \
	fi

# Clean build artifacts
clean:
	rm -rf $(BUILD_DIR)
	cd monocore && cargo clean && rm -rf build

# Build dependencies (libkrunfw and libkrun)
deps:
	./build_libkrun.sh --no-clean

# Help target
help:
	@echo "Available targets:"
	@echo "  build    - Build monocore and monokrun binaries"
	@echo "  install  - Install binaries and libraries to $(HOME)/.local/{bin,lib}"
	@echo "  clean    - Remove build artifacts"
	@echo "  deps     - Build and install dependencies"
	@echo "  help     - Show this help message"
	@echo ""
	@echo "Environment variables:"
	@echo "  OVERLAYFS=1  - Enable overlayfs feature flag"

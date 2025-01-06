# =============================================================================
# Monocore Makefile - Build, install, and run monocore components
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
# Installation Paths
# -----------------------------------------------------------------------------
HOME_LIB := $(HOME)/.local/lib
HOME_BIN := $(HOME)/.local/bin

# -----------------------------------------------------------------------------
# Build Paths and Directories
# -----------------------------------------------------------------------------
MONOCORE_RELEASE_BIN := target/release/monocore
MONOKRUN_RELEASE_BIN := target/release/monokrun
EXAMPLES_DIR := target/release/examples
BENCHES_DIR := target/release
BUILD_DIR := build

# -----------------------------------------------------------------------------
# Library Detection
# -----------------------------------------------------------------------------
ifeq ($(OS),Darwin)
	LIBKRUNFW_FILE := $(shell ls $(BUILD_DIR)/libkrunfw.*.dylib 2>/dev/null | head -n1)
	LIBKRUN_FILE := $(shell ls $(BUILD_DIR)/libkrun.*.dylib 2>/dev/null | head -n1)
else
	LIBKRUNFW_FILE := $(shell ls $(BUILD_DIR)/libkrunfw.so.* 2>/dev/null | head -n1)
	LIBKRUN_FILE := $(shell ls $(BUILD_DIR)/libkrun.so.* 2>/dev/null | head -n1)
endif

# -----------------------------------------------------------------------------
# Phony Targets Declaration
# -----------------------------------------------------------------------------
.PHONY: all build install clean deps example bench bin _run_example _run_bench _run_bin help uninstall

# -----------------------------------------------------------------------------
# Main Targets
# -----------------------------------------------------------------------------
all: build

build: deps $(MONOCORE_RELEASE_BIN) $(MONOKRUN_RELEASE_BIN)
	@cp $(MONOCORE_RELEASE_BIN) $(BUILD_DIR)/
	@cp $(MONOKRUN_RELEASE_BIN) $(BUILD_DIR)/
	@echo "Build artifacts copied to $(BUILD_DIR)/"

# -----------------------------------------------------------------------------
# Binary Building
# -----------------------------------------------------------------------------
$(MONOCORE_RELEASE_BIN): deps
	@mkdir -p $(BUILD_DIR)
	cd monocore
ifeq ($(OS),Darwin)
	RUSTFLAGS="-C link-args=-Wl,-rpath,@executable_path/../lib,-rpath,@executable_path" cargo build --release --bin monocore $(FEATURES)
	codesign --entitlements monocore.entitlements --force -s - $@
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,\$$ORIGIN/../lib,-rpath,\$$ORIGIN" cargo build --release --bin monocore $(FEATURES)
endif

$(MONOKRUN_RELEASE_BIN): deps
	cd monocore
ifeq ($(OS),Darwin)
	RUSTFLAGS="-C link-args=-Wl,-rpath,@executable_path/../lib,-rpath,@executable_path" cargo build --release --bin monokrun $(FEATURES)
	codesign --entitlements monocore.entitlements --force -s - $@
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,\$$ORIGIN/../lib,-rpath,\$$ORIGIN" cargo build --release --bin monokrun $(FEATURES)
endif

# -----------------------------------------------------------------------------
# Installation
# -----------------------------------------------------------------------------
install: build
	install -d $(HOME_BIN)
	install -d $(HOME_LIB)
	install -m 755 $(BUILD_DIR)/monocore $(HOME_BIN)/monocore
	install -m 755 $(BUILD_DIR)/monokrun $(HOME_BIN)/monokrun
	ln -sf $(HOME_BIN)/monocore $(HOME_BIN)/mc
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

# -----------------------------------------------------------------------------
# Development Tools
# -----------------------------------------------------------------------------
# Development binary target without RUSTFLAGS
$(MONOKRUN_RELEASE_BIN).dev: deps
	cd monocore && cargo build --release --bin monokrun $(FEATURES)
ifeq ($(OS),Darwin)
	codesign --entitlements monocore.entitlements --force -s - $(MONOKRUN_RELEASE_BIN)
endif
ifdef OVERLAYFS
	sudo setcap cap_sys_admin+ep $(MONOKRUN_RELEASE_BIN)
endif

# Run examples
example: $(MONOKRUN_RELEASE_BIN).dev
	@if [ -z "$(word 2,$(MAKECMDGOALS))" ]; then \
		echo "Usage: make example <example_name> [-- <args>]"; \
			exit 1; \
	fi
	@$(eval EXAMPLE_ARGS := $(filter-out example $(word 2,$(MAKECMDGOALS)) --, $(MAKECMDGOALS)))
	@$(MAKE) _run_example EXAMPLE_NAME=$(word 2,$(MAKECMDGOALS)) ARGS="$(EXAMPLE_ARGS)"

_run_example:
ifeq ($(OS),Darwin)
	cargo build --example $(EXAMPLE_NAME) --release
	codesign --entitlements monocore.entitlements --force -s - $(EXAMPLES_DIR)/$(EXAMPLE_NAME)
	DYLD_LIBRARY_PATH=$(BUILD_DIR):$$DYLD_LIBRARY_PATH $(EXAMPLES_DIR)/$(EXAMPLE_NAME) $(ARGS) || exit $$?
else
	cargo run --example $(EXAMPLE_NAME) --release -- $(ARGS) || exit $$?
endif

# -----------------------------------------------------------------------------
# Maintenance
# -----------------------------------------------------------------------------
clean:
	rm -rf $(BUILD_DIR)
	cd monocore && cargo clean && rm -rf build

uninstall:
	rm -f $(HOME_BIN)/monocore
	rm -f $(HOME_BIN)/monokrun
	rm -f $(HOME_BIN)/mc
	rm -f $(HOME_LIB)/libkrunfw.dylib
	rm -f $(HOME_LIB)/libkrun.dylib
	@if [ -n "$(LIBKRUNFW_FILE)" ]; then \
		rm -f $(HOME_LIB)/$(notdir $(LIBKRUNFW_FILE)); \
	fi
	@if [ -n "$(LIBKRUN_FILE)" ]; then \
		rm -f $(HOME_LIB)/$(notdir $(LIBKRUN_FILE)); \
	fi

deps:
	./scripts/build_libkrun.sh --no-clean

# Catch-all target to allow example names and arguments
%:
	@:

# -----------------------------------------------------------------------------
# Help Documentation
# -----------------------------------------------------------------------------
help:
	@echo "Monocore Makefile Help"
	@echo "======================"
	@echo
	@echo "Main Targets:"
	@echo "  make build                   - Build monocore and monokrun binaries"
	@echo "  make install                 - Install binaries and libraries to ~/.local/{bin,lib}"
	@echo "  make uninstall               - Remove all installed components"
	@echo "  make clean                   - Remove build artifacts"
	@echo "  make deps                    - Build and install dependencies"
	@echo
	@echo "Development Tools:"
	@echo "  make example <name> [-- <args>]  - Build and run an example"
	@echo "    Example: make example microvm_shell -- arg1 arg2"
	@echo
	@echo "Note: For commands that accept arguments, use -- to separate them"
	@echo "      from the make target name."

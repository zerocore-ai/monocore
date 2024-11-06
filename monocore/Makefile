# System and architecture variables
OS := $(shell uname -s)
ARCH := $(shell uname -m)
ifeq ($(ARCH),aarch64)
	ARCH := arm64
endif
ifeq ($(ARCH),x86_64)
	ARCH := x86_64
endif

PREFIX ?= /usr/local
MONOCORE_RELEASE_BIN := ../target/release/monocore
MONOKRUN_RELEASE_BIN := ../target/release/monokrun
EXAMPLES_DIR := ../target/release/examples
FIXTURES_DIR := fixtures
DISTRO_ROOTFS := rootfs-alpine
BUILD_DIR := build
BENCHES_DIR := ../target/release

# Library paths
DARWIN_LIB_PATH := /usr/local/lib
LINUX_LIB_PATH := /usr/local/lib64

# Phony targets
.PHONY: all install clean example unpack_rootfs bench build_monokrun

# Default target
all: $(MONOCORE_RELEASE_BIN)

# Build monokrun in release mode
build_monokrun: $(MONOKRUN_RELEASE_BIN)

$(MONOKRUN_RELEASE_BIN):
	cargo build --release --bin monokrun
ifeq ($(OS),Darwin)
	codesign --entitlements monocore.entitlements --force -s - $@
endif

# Build the release binary
$(MONOCORE_RELEASE_BIN):
	cargo build --release -p monocore
ifeq ($(OS),Darwin)
	codesign --entitlements monocore.entitlements --force -s - $@
endif

# Install the binary
install: $(MONOCORE_RELEASE_BIN)
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $< $(DESTDIR)$(PREFIX)/bin

# Clean build artifacts
clean:
	cargo clean
	rm -rf $(BUILD_DIR)

# Unpack rootfs
unpack_rootfs: $(BUILD_DIR)/$(DISTRO_ROOTFS)-$(ARCH)

$(BUILD_DIR)/$(DISTRO_ROOTFS)-$(ARCH): $(FIXTURES_DIR)/$(DISTRO_ROOTFS)-$(ARCH).tar.gz
	mkdir -p $(BUILD_DIR)/$(DISTRO_ROOTFS)-$(ARCH)
	tar -xzf $< -C $(BUILD_DIR)
	touch $@

# Run examples
example: build_monokrun unpack_rootfs
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
	RUST_LOG=debug DYLD_LIBRARY_PATH=$(DARWIN_LIB_PATH):$$DYLD_LIBRARY_PATH $(EXAMPLES_DIR)/$(EXAMPLE_NAME) $(ARGS) || exit $$?
else
	LD_LIBRARY_PATH=$(LINUX_LIB_PATH):$$LD_LIBRARY_PATH cargo run --example $(EXAMPLE_NAME) --release -- $(ARGS) || exit $$?
endif

# Run benchmarks
bench:
	@if [ -z "$(word 2,$(MAKECMDGOALS))" ]; then \
		echo "Usage: make bench <benchmark_name>"; \
			exit 1; \
	fi
	@$(MAKE) _run_bench BENCH_NAME=$(word 2,$(MAKECMDGOALS))

_run_bench:
	cargo bench --bench $(BENCH_NAME)

# Catch-all target to allow example names and arguments
%:
	@:

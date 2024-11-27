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
BUILD_DIR := build
BENCHES_DIR := ../target/release

# Library paths
DARWIN_LIB_PATH := /usr/local/lib
LINUX_LIB_PATH := /usr/local/lib64

# Phony targets
.PHONY: all install clean example bench monokrun bin _run_bin monocore deps

# Default target
all: monocore

# Build monokrun in release mode
monokrun: $(MONOKRUN_RELEASE_BIN)

$(MONOCORE_RELEASE_BIN): deps
	@mkdir -p $(BUILD_DIR)
ifeq ($(OS),Darwin)
	cargo build --release --bin monocore $(FEATURES)
	codesign --entitlements monocore/monocore.entitlements --force -s - $@
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,$(LINUX_LIB_PATH)" cargo build --release --bin monocore $(FEATURES)
ifdef OVERLAYFS
	sudo setcap cap_sys_admin+ep $@
endif
endif

$(MONOKRUN_RELEASE_BIN): deps
ifeq ($(OS),Darwin)
	cargo build --release --bin monokrun $(FEATURES)
	codesign --entitlements monocore/monocore.entitlements --force -s - $@
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,$(LINUX_LIB_PATH)" cargo build --release --bin monokrun $(FEATURES)
ifdef OVERLAYFS
	sudo setcap cap_sys_admin+ep $@
endif
endif

# Install the binaries
install: $(MONOCORE_RELEASE_BIN) $(MONOKRUN_RELEASE_BIN)
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $(MONOCORE_RELEASE_BIN) $(DESTDIR)$(PREFIX)/bin/monocore
	install -m 755 $(MONOKRUN_RELEASE_BIN) $(DESTDIR)$(PREFIX)/bin/monokrun

# Clean build artifacts
clean:
	cargo clean
	rm -rf $(BUILD_DIR)

# Run examples
example: monokrun
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
	DYLD_LIBRARY_PATH=$(DARWIN_LIB_PATH):$$RUST_DYLD_LIBRARY_PATH $(EXAMPLES_DIR)/$(EXAMPLE_NAME) $(ARGS) || exit $$?
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,$(LINUX_LIB_PATH)" LD_LIBRARY_PATH=$(LINUX_LIB_PATH):$$LD_LIBRARY_PATH cargo run --example $(EXAMPLE_NAME) --release -- $(ARGS) || exit $$?
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

# Run binaries from bin/
bin: monokrun
	@if [ -z "$(word 2,$(MAKECMDGOALS))" ]; then \
		echo "Usage: make bin <binary_name> [-- <args>]"; \
			exit 1; \
	fi
	@$(eval BIN_ARGS := $(filter-out bin $(word 2,$(MAKECMDGOALS)) --, $(MAKECMDGOALS)))
	@$(MAKE) _run_bin BIN_NAME=$(word 2,$(MAKECMDGOALS)) ARGS="$(BIN_ARGS)"

_run_bin:
ifeq ($(OS),Darwin)
	cargo build --bin $(BIN_NAME) --release
	codesign --entitlements monocore.entitlements --force -s - $(BENCHES_DIR)/$(BIN_NAME)
	DYLD_LIBRARY_PATH=$(DARWIN_LIB_PATH):$$RUST_DYLD_LIBRARY_PATH $(BENCHES_DIR)/$(BIN_NAME) $(ARGS) || exit $$?
else
	RUSTFLAGS="-C link-args=-Wl,-rpath,$(LINUX_LIB_PATH)" LD_LIBRARY_PATH=$(LINUX_LIB_PATH):$$LD_LIBRARY_PATH cargo run --bin $(BIN_NAME) --release -- $(ARGS) || exit $$?
endif

# Build dependencies (libkrunfw and libkrun)
deps:
	@if [ ! -f "$(DARWIN_LIB_PATH)/libkrun.dylib" ] && [ ! -f "$(LINUX_LIB_PATH)/libkrun.so" ]; then \
		./build_libkrun.sh; \
	fi

# Build the CLI (includes both monocore and monokrun)
monocore: $(MONOCORE_RELEASE_BIN) $(MONOKRUN_RELEASE_BIN)
	@mkdir -p $(BUILD_DIR)

# Catch-all target to allow example names and arguments
%:
	@:

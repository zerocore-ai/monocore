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
MONOCORE_RELEASE_BIN := target/release/monocore
MONOKRUN_RELEASE_BIN := target/release/monokrun
BUILD_DIR := build

# Library paths
DARWIN_LIB_PATH := /usr/local/lib
LINUX_LIB_PATH := /usr/local/lib64

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

$(MONOCORE_RELEASE_BIN): deps
	@mkdir -p $(BUILD_DIR)
	cd monocore
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
	cd monocore
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
install: build
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $(MONOCORE_RELEASE_BIN) $(DESTDIR)$(PREFIX)/bin/monocore
	install -m 755 $(MONOKRUN_RELEASE_BIN) $(DESTDIR)$(PREFIX)/bin/monokrun

# Clean build artifacts
clean:
	rm -rf $(BUILD_DIR)
	cd monocore && cargo clean && rm -rf build

# Build dependencies (libkrunfw and libkrun)
deps:
	@if [ ! -f "$(DARWIN_LIB_PATH)/libkrun.dylib" ] && [ ! -f "$(LINUX_LIB_PATH)/libkrun.so" ]; then \
		./build_libkrun.sh; \
	fi

# Help target
help:
	@echo "Available targets:"
	@echo "  build    - Build monocore and monokrun binaries"
	@echo "  install  - Install binaries to $(PREFIX)/bin"
	@echo "  clean    - Remove build artifacts"
	@echo "  deps     - Build and install dependencies"
	@echo "  help     - Show this help message"
	@echo ""
	@echo "Environment variables:"
	@echo "  OVERLAYFS=1  - Enable overlayfs feature flag"

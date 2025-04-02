CC = gcc
CFLAGS = -Wall -Wextra -I./include
LDFLAGS =
SRC_DIR = src
TEST_DIR = test
BUILD_DIR = build
LIB_DIR = lib
DIST_DIR = dist

.PHONY: all clean test lib dist

all: test

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(LIB_DIR):
	mkdir -p $(LIB_DIR)

$(DIST_DIR):
	mkdir -p $(DIST_DIR)

# Build the library
lib: $(LIB_DIR) $(BUILD_DIR) $(SRC_DIR)/microsandbox.c
	$(CC) $(CFLAGS) -c $(SRC_DIR)/microsandbox.c -o $(BUILD_DIR)/microsandbox.o
	ar rcs $(LIB_DIR)/libmicrosandbox.a $(BUILD_DIR)/microsandbox.o

# Build tests
test: $(BUILD_DIR) lib $(TEST_DIR)/test_microsandbox.c
	$(CC) $(CFLAGS) $(TEST_DIR)/test_microsandbox.c -o $(BUILD_DIR)/test_microsandbox $(LIB_DIR)/libmicrosandbox.a
	$(BUILD_DIR)/test_microsandbox

# Create a distribution tarball
dist: $(DIST_DIR) lib
	mkdir -p $(DIST_DIR)/microsandbox-0.1.0/include
	mkdir -p $(DIST_DIR)/microsandbox-0.1.0/lib
	cp include/microsandbox.h $(DIST_DIR)/microsandbox-0.1.0/include/
	cp $(LIB_DIR)/libmicrosandbox.a $(DIST_DIR)/microsandbox-0.1.0/lib/
	cp README.md $(DIST_DIR)/microsandbox-0.1.0/
	cd $(DIST_DIR) && tar -czvf microsandbox-0.1.0.tar.gz microsandbox-0.1.0

# Clean build artifacts
clean:
	rm -rf $(BUILD_DIR) $(LIB_DIR) $(DIST_DIR)

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../include/microsandbox.h"

// Simple test function
int test_greet() {
    const char* test_name = "Test";
    char* result = microsandbox_greet(test_name);

    // Check if the result contains the expected text
    int success = (result != NULL && strstr(result, "Hello, Test!") != NULL);

    // Clean up
    free(result);

    return success;
}

int main() {
    if (test_greet()) {
        printf("Test passed!\n");
        return 0;
    } else {
        printf("Test failed!\n");
        return 1;
    }
}

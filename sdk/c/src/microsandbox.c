#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../include/microsandbox.h"

char* microsandbox_greet(const char* name) {
    // Calculate the length of the message
    const char* prefix = "Hello, ";
    const char* suffix = "! Welcome to Microsandbox!";
    size_t total_length = strlen(prefix) + strlen(name) + strlen(suffix) + 1; // +1 for null terminator

    // Allocate memory for the message
    char* message = (char*)malloc(total_length);
    if (message == NULL) {
        return NULL; // Memory allocation failed
    }

    // Construct the message
    sprintf(message, "%s%s%s", prefix, name, suffix);

    // Print the message
    printf("%s\n", message);

    return message;
}

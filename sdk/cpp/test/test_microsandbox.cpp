#include <iostream>
#include <string>
#include <cassert>
#include <microsandbox/microsandbox.hpp>

bool test_greet() {
    std::string result = microsandbox::greet("Test");

    // Check if the result contains the expected text
    bool success = (result.find("Hello, Test!") != std::string::npos);

    return success;
}

int main() {
    if (test_greet()) {
        std::cout << "Test passed!" << std::endl;
        return 0;
    } else {
        std::cout << "Test failed!" << std::endl;
        return 1;
    }
}

#include <iostream>
#include <microsandbox/microsandbox.hpp>
namespace microsandbox {
    std::string greet(const std::string& name) {
        std::string message = "Hello, " + name + "! Welcome to Microsandbox!";
        std::cout << message << std::endl;
        return message;
    }
}

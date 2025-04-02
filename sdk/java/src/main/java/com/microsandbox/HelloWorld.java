package com.microsandbox;

/**
 * Hello World class for the Microsandbox SDK.
 */
public class HelloWorld {

    /**
     * Returns a greeting message for the given name.
     *
     * @param name The name to greet
     * @return A greeting message
     */
    public static String greet(String name) {
        String message = "Hello, " + name + "! Welcome to Microsandbox!";
        System.out.println(message);
        return message;
    }
}

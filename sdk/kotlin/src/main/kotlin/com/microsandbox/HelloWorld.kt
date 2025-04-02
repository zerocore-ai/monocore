package com.microsandbox

/**
 * Hello World class for the Microsandbox SDK.
 */
object HelloWorld {

    /**
     * Returns a greeting message for the given name.
     *
     * @param name The name to greet
     * @return A greeting message
     */
    fun greet(name: String): String {
        val message = "Hello, $name! Welcome to Microsandbox!"
        println(message)
        return message
    }
}

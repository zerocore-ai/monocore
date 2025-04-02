package com.microsandbox

import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * Test class for the HelloWorld object.
 */
class HelloWorldTest {

    @Test
    fun testGreet() {
        val result = HelloWorld.greet("Test")
        assertTrue(result.contains("Hello, Test!"))
    }
}

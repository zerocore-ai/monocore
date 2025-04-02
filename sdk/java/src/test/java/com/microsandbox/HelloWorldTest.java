package com.microsandbox;

import org.junit.Test;
import static org.junit.Assert.*;

/**
 * Test class for the HelloWorld class.
 */
public class HelloWorldTest {

    @Test
    public void testGreet() {
        String result = HelloWorld.greet("Test");
        assertTrue(result.contains("Hello, Test!"));
    }
}

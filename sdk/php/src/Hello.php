<?php

namespace Microsandbox;

/**
 * Hello class for the Microsandbox SDK.
 */
class Hello
{
    /**
     * Returns a greeting message for the given name.
     *
     * @param string $name The name to greet
     * @return string A greeting message
     */
    public static function greet(string $name): string
    {
        $message = "Hello, {$name}! Welcome to Microsandbox!";
        echo $message . PHP_EOL;
        return $message;
    }
}

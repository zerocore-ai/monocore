<?php

namespace Microsandbox\Tests;

use Microsandbox\Hello;
use PHPUnit\Framework\TestCase;

class HelloTest extends TestCase
{
    public function testGreet(): void
    {
        $result = Hello::greet('Test');
        $this->assertStringContainsString('Hello, Test!', $result);
    }
}

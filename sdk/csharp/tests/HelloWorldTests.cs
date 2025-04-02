using System;
using Xunit;
using Microsandbox;

namespace Microsandbox.Tests
{
    public class HelloWorldTests
    {
        [Fact]
        public void Greet_ReturnsCorrectMessage()
        {
            // Arrange & Act
            var result = HelloWorld.Greet("Test");

            // Assert
            Assert.Contains("Hello, Test!", result);
        }
    }
}

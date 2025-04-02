using System;

namespace Microsandbox
{
    /// <summary>
    /// Hello World class for the Microsandbox SDK.
    /// </summary>
    public static class HelloWorld
    {
        /// <summary>
        /// Returns a greeting message for the given name.
        /// </summary>
        /// <param name="name">The name to greet</param>
        /// <returns>A greeting message</returns>
        public static string Greet(string name)
        {
            var message = $"Hello, {name}! Welcome to Microsandbox!";
            Console.WriteLine(message);
            return message;
        }
    }
}

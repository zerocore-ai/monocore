namespace Microsandbox

/// Greeter module for the Microsandbox SDK.
module Greeter =
    /// Returns a greeting message for the given name.
    ///
    /// Parameters:
    ///   name - The name to greet
    ///
    /// Returns:
    ///   A greeting message
    let greet (name: string) =
        let message = sprintf "Hello, %s! Welcome to Microsandbox!" name
        printfn "%s" message
        message

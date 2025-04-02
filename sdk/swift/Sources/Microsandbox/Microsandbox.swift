/// A minimal SDK for the Microsandbox project.
public struct Microsandbox {
    /// Returns a greeting message for the given name.
    ///
    /// - Parameter name: The name to greet
    /// - Returns: A greeting message
    public static func greet(_ name: String) -> String {
        let message = "Hello, \(name)! Welcome to Microsandbox!"
        print(message)
        return message
    }
}

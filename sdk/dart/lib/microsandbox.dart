/// A minimal SDK for the Microsandbox project.
library microsandbox;

/// Returns a greeting message for the given name.
///
/// Example:
/// ```dart
/// var message = greet('World');
/// print(message); // Prints: Hello, World! Welcome to Microsandbox!
/// ```
String greet(String name) {
  final message = 'Hello, $name! Welcome to Microsandbox!';
  print(message);
  return message;
}

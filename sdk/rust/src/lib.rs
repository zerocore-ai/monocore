//! Microsandbox Rust SDK
//!
//! A minimal SDK for the Microsandbox project.

/// Returns a greeting message for the given name.
///
/// # Examples
///
/// ```
/// let message = microsandbox::greet("World");
/// assert!(message.contains("Hello, World!"));
/// ```
pub fn greet(name: &str) -> String {
    let message = format!("Hello, {}! Welcome to Microsandbox!", name);
    println!("{}", message);
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        let result = greet("Test");
        assert!(result.contains("Hello, Test!"));
    }
}

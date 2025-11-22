//! # Rust Template
//!
//! A minimal Rust library template demonstrating basic module structure and documentation.
//!
//! ## Example
//!
//! ```
//! use rust_template::greet;
//!
//! let message = greet("Rust");
//! assert_eq!(message, "Hello, Rust!");
//! ```

/// Formats a greeting message.
fn format_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}

/// Greets the given name.
///
/// # Arguments
///
/// * `name` - The name to greet
///
/// # Returns
///
/// A greeting string
///
/// # Examples
///
/// ```
/// use rust_template::greet;
///
/// let message = greet("World");
/// assert_eq!(message, "Hello, World!");
/// ```
pub fn greet(name: &str) -> String {
    format_greeting(name)
}

/// A trait for things that can greet.
pub trait Greeter {
    /// Greet someone.
    fn greet(&self, name: &str) -> String;
}

/// A friendly greeter that uses exclamation marks.
pub struct FriendlyGreeter;

impl Greeter for FriendlyGreeter {
    fn greet(&self, name: &str) -> String {
        format!("Hello, {}!", name)
    }
}

/// A formal greeter that uses periods.
pub struct FormalGreeter;

impl Greeter for FormalGreeter {
    fn greet(&self, name: &str) -> String {
        format!("Good day, {}.", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("Rust"), "Hello, Rust!");
    }

    #[test]
    fn test_friendly_greeter() {
        let greeter = FriendlyGreeter;
        assert_eq!(greeter.greet("World"), "Hello, World!");
    }

    #[test]
    fn test_formal_greeter() {
        let greeter = FormalGreeter;
        assert_eq!(greeter.greet("World"), "Good day, World.");
    }
}

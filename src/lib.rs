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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("Rust"), "Hello, Rust!");
    }
}

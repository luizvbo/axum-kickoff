//! Test data builders
//!
//! Adapted from crates.io's builders to provide a fluent API
//! for creating test data. This module will be extended with
//! domain-specific builders as the application grows.

use std::marker::PhantomData;

/// Trait for builder pattern
pub trait Builder {
    type Output;

    /// Build the final object
    fn build(self) -> Self::Output;
}

/// Generic builder for creating test data
///
/// This provides a foundation that can be extended with
/// domain-specific builders (UserBuilder, PostBuilder, etc.)
pub struct TestBuilder<T> {
    _phantom: PhantomData<T>,
}

impl<T> TestBuilder<T> {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for TestBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Example: UserBuilder (to be implemented when User model exists)
//
// pub struct UserBuilder {
//     username: Option<String>,
//     email: Option<String>,
//     // ... other fields
// }
//
// impl UserBuilder {
//     pub fn new() -> Self {
//         Self {
//             username: None,
//             email: None,
//         }
//     }
//
//     pub fn username(mut self, username: impl Into<String>) -> Self {
//         self.username = Some(username.into());
//         self
//     }
//
//     pub fn email(mut self, email: impl Into<String>) -> Self {
//         self.email = Some(email.into());
//         self
//     }
//
//     pub fn build(self) -> User {
//         User {
//             username: self.username.unwrap_or_else(|| "test_user".to_string()),
//             email: self.email.unwrap_or_else(|| "test@example.com".to_string()),
//         }
//     }
// }
//
// impl Default for UserBuilder {
//     fn default() -> Self {
//         Self::new()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creation() {
        let _builder: TestBuilder<()> = TestBuilder::new();
        // This is a placeholder test - real tests will be added
        // when domain-specific builders are implemented
    }
}

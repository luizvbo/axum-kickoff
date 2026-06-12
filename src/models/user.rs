//! User model
//!
//! Represents a user in the system with authentication and profile information.

use toasty::Model;

#[derive(Debug, Model)]
pub struct User {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// Unique email address for authentication
    #[unique]
    pub email: String,

    /// User's display name
    pub name: String,

    /// Hashed password (never store plain text passwords)
    pub password_hash: String,

    /// Whether the user account is active
    pub is_active: bool,

    /// Timestamp when the user was created
    pub created_at: jiff::Timestamp,

    /// Timestamp when the user was last updated
    pub updated_at: jiff::Timestamp,
}

impl User {
    /// Create a new user with the given email, name, and password hash
    pub fn new(email: String, name: String, password_hash: String) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            email,
            name,
            password_hash,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the updated_at timestamp to the current time
    pub fn touch(&mut self) {
        self.updated_at = jiff::Timestamp::now();
    }
}

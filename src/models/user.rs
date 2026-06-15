//! User model
//!
//! Represents a user in the system with authentication and profile information.
//! This model is designed for GitHub OAuth authentication.

use toasty::Model;

#[derive(Debug, Model)]
pub struct User {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// GitHub account ID (unique identifier from GitHub)
    #[unique]
    pub gh_id: i64,

    /// GitHub username (login)
    pub gh_login: String,

    /// User's display name (from GitHub profile)
    pub name: Option<String>,

    /// User's email (from GitHub profile)
    pub email: Option<String>,

    /// Avatar URL from GitHub
    pub gh_avatar: Option<String>,

    /// Whether the user account is active
    pub is_active: bool,

    /// Timestamp when the user was created
    pub created_at: jiff::Timestamp,

    /// Timestamp when the user was last updated
    pub updated_at: jiff::Timestamp,
}

impl User {
    /// Create a new user from GitHub OAuth data
    pub fn new_from_github(gh_id: i64, gh_login: String, name: Option<String>, email: Option<String>, gh_avatar: Option<String>) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            gh_id,
            gh_login,
            name,
            email,
            gh_avatar,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the updated_at timestamp to the current time
    pub fn touch(&mut self) {
        self.updated_at = jiff::Timestamp::now();
    }

    /// Update user info from GitHub profile data
    pub fn update_from_github(&mut self, name: Option<String>, email: Option<String>, gh_avatar: Option<String>) {
        self.name = name;
        self.email = email;
        self.gh_avatar = gh_avatar;
        self.touch();
    }
}

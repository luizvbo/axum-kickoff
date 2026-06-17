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

    /// Reason for account lock (if locked)
    pub account_lock_reason: Option<String>,

    /// Timestamp when account lock expires (if locked)
    pub account_lock_until: Option<jiff::Timestamp>,

    /// Timestamp when the user was created
    pub created_at: jiff::Timestamp,

    /// Timestamp when the user was last updated
    pub updated_at: jiff::Timestamp,
}

impl User {
    /// Create a new user from GitHub OAuth data
    pub fn new_from_github(
        gh_id: i64,
        gh_login: String,
        name: Option<String>,
        email: Option<String>,
        gh_avatar: Option<String>,
    ) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            gh_id,
            gh_login,
            name,
            email,
            gh_avatar,
            is_active: true,
            account_lock_reason: None,
            account_lock_until: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the updated_at timestamp to the current time
    pub fn touch(&mut self) {
        self.updated_at = jiff::Timestamp::now();
    }

    /// Update user info from GitHub profile data
    pub fn update_from_github(
        &mut self,
        name: Option<String>,
        email: Option<String>,
        gh_avatar: Option<String>,
    ) {
        self.name = name;
        self.email = email;
        self.gh_avatar = gh_avatar;
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_from_github() {
        let gh_id = 12345;
        let gh_login = "testuser".to_string();
        let name = Some("Test User".to_string());
        let email = Some("test@example.com".to_string());
        let gh_avatar = Some("https://example.com/avatar.png".to_string());

        let user = User::new_from_github(
            gh_id,
            gh_login.clone(),
            name.clone(),
            email.clone(),
            gh_avatar.clone(),
        );

        assert_eq!(user.gh_id, gh_id);
        assert_eq!(user.gh_login, gh_login);
        assert_eq!(user.name, name);
        assert_eq!(user.email, email);
        assert_eq!(user.gh_avatar, gh_avatar);
        assert!(user.is_active);
        assert!(user.account_lock_reason.is_none());
        assert!(user.account_lock_until.is_none());
        assert_eq!(user.id, 0); // Will be auto-generated
    }

    #[test]
    fn test_new_from_github_minimal() {
        let user = User::new_from_github(12345, "testuser".to_string(), None, None, None);

        assert_eq!(user.gh_id, 12345);
        assert_eq!(user.gh_login, "testuser");
        assert!(user.name.is_none());
        assert!(user.email.is_none());
        assert!(user.gh_avatar.is_none());
        assert!(user.is_active);
    }

    #[test]
    fn test_touch() {
        let mut user = User::new_from_github(12345, "testuser".to_string(), None, None, None);
        let original_updated_at = user.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        user.touch();

        assert!(user.updated_at > original_updated_at);
    }

    #[test]
    fn test_update_from_github() {
        let mut user = User::new_from_github(12345, "testuser".to_string(), None, None, None);
        let original_updated_at = user.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        user.update_from_github(
            Some("Updated Name".to_string()),
            Some("updated@example.com".to_string()),
            Some("https://example.com/new-avatar.png".to_string()),
        );

        assert_eq!(user.name, Some("Updated Name".to_string()));
        assert_eq!(user.email, Some("updated@example.com".to_string()));
        assert_eq!(
            user.gh_avatar,
            Some("https://example.com/new-avatar.png".to_string())
        );
        assert!(user.updated_at > original_updated_at);
    }

    #[test]
    fn test_update_from_github_partial() {
        let mut user = User::new_from_github(
            12345,
            "testuser".to_string(),
            Some("Original Name".to_string()),
            Some("original@example.com".to_string()),
            Some("https://example.com/avatar.png".to_string()),
        );

        user.update_from_github(None, None, None);

        // Fields should be cleared to None
        assert!(user.name.is_none());
        assert!(user.email.is_none());
        assert!(user.gh_avatar.is_none());
    }
}

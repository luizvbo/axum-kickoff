//! User model
//!
//! Represents a user in the system with authentication and profile
//! information. Identities are keyed by `(provider, provider_user_id)` —
//! e.g. `("github", "583231")` — so accounts from different OAuth providers
//! stay distinct (no implicit linking by email, which would be an
//! account-takeover risk with providers that return unverified emails).

use toasty::Model;

#[derive(Clone, Debug, Model)]
#[unique(provider, provider_user_id)]
pub struct User {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// OAuth provider slug that owns this identity (`"github"`, `"google"`,
    /// `"facebook"`)
    pub provider: String,

    /// Unique user ID assigned by the OAuth provider
    pub provider_user_id: String,

    /// Username/handle from the provider profile
    pub login: String,

    /// User's display name (from the provider profile)
    pub name: Option<String>,

    /// User's email (from the provider profile)
    pub email: Option<String>,

    /// Avatar URL from the provider profile
    pub avatar_url: Option<String>,

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
    /// Create a new user from OAuth profile data
    pub fn new(
        provider: &str,
        provider_user_id: String,
        login: String,
        name: Option<String>,
        email: Option<String>,
        avatar_url: Option<String>,
    ) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            provider: provider.to_string(),
            provider_user_id,
            login,
            name,
            email,
            avatar_url,
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

    /// Update user info from an OAuth profile
    pub fn update_from_oauth(
        &mut self,
        login: String,
        name: Option<String>,
        email: Option<String>,
        avatar_url: Option<String>,
    ) {
        self.login = login;
        self.name = name;
        self.email = email;
        self.avatar_url = avatar_url;
        self.touch();
    }

    /// Returns true if the account is currently locked.
    pub fn is_locked(&self) -> bool {
        self.account_lock_until
            .as_ref()
            .is_some_and(|lock_until| *lock_until > jiff::Timestamp::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user() -> User {
        User::new(
            "github",
            "12345".to_string(),
            "testuser".to_string(),
            Some("Test User".to_string()),
            Some("test@example.com".to_string()),
            Some("https://example.com/avatar.png".to_string()),
        )
    }

    #[test]
    fn test_new() {
        let user = test_user();

        assert_eq!(user.provider, "github");
        assert_eq!(user.provider_user_id, "12345");
        assert_eq!(user.login, "testuser");
        assert_eq!(user.name.as_deref(), Some("Test User"));
        assert_eq!(user.email.as_deref(), Some("test@example.com"));
        assert_eq!(
            user.avatar_url.as_deref(),
            Some("https://example.com/avatar.png")
        );
        assert!(user.is_active);
        assert!(user.account_lock_reason.is_none());
        assert!(user.account_lock_until.is_none());
        assert_eq!(user.id, 0); // Will be auto-generated
    }

    #[test]
    fn test_new_minimal() {
        let user = User::new(
            "google",
            "sub-1".to_string(),
            "user".to_string(),
            None,
            None,
            None,
        );

        assert_eq!(user.provider, "google");
        assert_eq!(user.provider_user_id, "sub-1");
        assert_eq!(user.login, "user");
        assert!(user.name.is_none());
        assert!(user.email.is_none());
        assert!(user.avatar_url.is_none());
        assert!(user.is_active);
    }

    #[test]
    fn test_touch() {
        let mut user = test_user();
        let original_updated_at = user.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        user.touch();

        assert!(user.updated_at > original_updated_at);
    }

    #[test]
    fn test_update_from_oauth() {
        let mut user = test_user();
        let original_updated_at = user.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        user.update_from_oauth(
            "new_login".to_string(),
            Some("Updated Name".to_string()),
            Some("updated@example.com".to_string()),
            Some("https://example.com/new-avatar.png".to_string()),
        );

        assert_eq!(user.login, "new_login");
        assert_eq!(user.name.as_deref(), Some("Updated Name"));
        assert_eq!(user.email.as_deref(), Some("updated@example.com"));
        assert_eq!(
            user.avatar_url.as_deref(),
            Some("https://example.com/new-avatar.png")
        );
        assert!(user.updated_at > original_updated_at);
    }

    #[test]
    fn test_is_locked() {
        let now = jiff::Timestamp::now();
        let future = now
            .checked_add(jiff::SignedDuration::from_hours(1))
            .unwrap();
        let past = now
            .checked_add(jiff::SignedDuration::from_hours(-1))
            .unwrap();

        let mut locked_user = User::new(
            "github",
            "1".to_string(),
            "locked".to_string(),
            None,
            None,
            None,
        );
        locked_user.account_lock_until = Some(future);
        locked_user.account_lock_reason = Some("Banned".to_string());
        assert!(locked_user.is_locked());

        let mut expired_lock_user = User::new(
            "github",
            "2".to_string(),
            "expired".to_string(),
            None,
            None,
            None,
        );
        expired_lock_user.account_lock_until = Some(past);
        expired_lock_user.account_lock_reason = Some("Old ban".to_string());
        assert!(!expired_lock_user.is_locked());

        let unlocked_user = User::new(
            "github",
            "3".to_string(),
            "unlocked".to_string(),
            None,
            None,
            None,
        );
        assert!(!unlocked_user.is_locked());
    }

    #[test]
    fn test_update_from_oauth_partial() {
        let mut user = test_user();

        user.update_from_oauth("testuser".to_string(), None, None, None);

        // Fields should be cleared to None
        assert!(user.name.is_none());
        assert!(user.email.is_none());
        assert!(user.avatar_url.is_none());
    }
}

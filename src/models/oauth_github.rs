//! OAuth GitHub model
//!
//! Stores GitHub OAuth token and account information for users.
//! This allows the application to make authenticated requests to GitHub
//! on behalf of the user (e.g., for syncing profile data).

use toasty::Model;

#[derive(Debug, Model)]
pub struct OauthGithub {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// Foreign key to the user
    #[unique]
    pub user_id: u64,

    /// GitHub account ID
    pub account_id: i64,

    /// GitHub username (login)
    pub login: String,

    /// Encrypted GitHub access token
    pub encrypted_token: Vec<u8>,

    /// Timestamp when this record was last synced with GitHub
    pub last_sync: jiff::Timestamp,

    /// Timestamp when the record was created
    pub created_at: jiff::Timestamp,

    /// Timestamp when the record was last updated
    pub updated_at: jiff::Timestamp,
}

impl OauthGithub {
    /// Create a new OAuth GitHub record
    pub fn new(user_id: u64, account_id: i64, login: String, encrypted_token: Vec<u8>) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            user_id,
            account_id,
            login,
            encrypted_token,
            last_sync: now,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the last_sync timestamp to the current time
    pub fn touch_sync(&mut self) {
        self.last_sync = jiff::Timestamp::now();
        self.updated_at = jiff::Timestamp::now();
    }

    /// Update the login and timestamp
    pub fn update_login(&mut self, login: String) {
        self.login = login;
        self.touch_sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_oauth_github() {
        let user_id = 123;
        let account_id = 456;
        let login = "testuser".to_string();
        let encrypted_token = vec![1, 2, 3, 4];

        let oauth = OauthGithub::new(user_id, account_id, login.clone(), encrypted_token.clone());

        assert_eq!(oauth.user_id, user_id);
        assert_eq!(oauth.account_id, account_id);
        assert_eq!(oauth.login, login);
        assert_eq!(oauth.encrypted_token, encrypted_token);
        assert_eq!(oauth.id, 0); // Will be auto-generated
    }

    #[test]
    fn test_touch_sync() {
        let mut oauth = OauthGithub::new(1, 2, "test".to_string(), vec![1, 2, 3]);
        let original_last_sync = oauth.last_sync;
        let original_updated_at = oauth.updated_at;

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));
        oauth.touch_sync();

        assert!(oauth.last_sync > original_last_sync);
        assert!(oauth.updated_at > original_updated_at);
    }

    #[test]
    fn test_update_login() {
        let mut oauth = OauthGithub::new(1, 2, "old_login".to_string(), vec![1, 2, 3]);
        let original_last_sync = oauth.last_sync;
        let original_updated_at = oauth.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        oauth.update_login("new_login".to_string());

        assert_eq!(oauth.login, "new_login");
        assert!(oauth.last_sync > original_last_sync);
        assert!(oauth.updated_at > original_updated_at);
    }
}

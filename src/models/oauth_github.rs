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

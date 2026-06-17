//! Test data builders
//!
//! Provides a fluent API for creating test data, inserting directly
//! into the database using Toasty ORM.

use crate::models::{ApiToken, ResourceScope, ActionScope, User};
use crate::util::PlainToken;
use secrecy::ExposeSecret;
use std::sync::atomic::{AtomicI64, Ordering};

static NEXT_GH_ID: AtomicI64 = AtomicI64::new(1000);

/// Builder for User models
pub struct UserBuilder {
    gh_id: i64,
    gh_login: String,
    name: Option<String>,
    email: Option<String>,
    gh_avatar: Option<String>,
    is_active: bool,
    account_lock_reason: Option<String>,
    account_lock_until: Option<jiff::Timestamp>,
}

impl UserBuilder {
    pub fn new(gh_login: &str) -> Self {
        Self {
            gh_id: NEXT_GH_ID.fetch_add(1, Ordering::SeqCst),
            gh_login: gh_login.to_string(),
            name: None,
            email: None,
            gh_avatar: None,
            is_active: true,
            account_lock_reason: None,
            account_lock_until: None,
        }
    }

    pub fn gh_id(mut self, gh_id: i64) -> Self {
        self.gh_id = gh_id;
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn email(mut self, email: &str) -> Self {
        self.email = Some(email.to_string());
        self
    }

    pub fn inactive(mut self) -> Self {
        self.is_active = false;
        self
    }

    pub fn locked(mut self, reason: &str, until: Option<jiff::Timestamp>) -> Self {
        self.account_lock_reason = Some(reason.to_string());
        self.account_lock_until = until;
        self
    }

    /// Build and insert the user into the database using Toasty ORM
    pub async fn build(self, db: &mut toasty::Db) -> anyhow::Result<User> {
        let user = toasty::create!(User {
            gh_id: self.gh_id,
            gh_login: self.gh_login,
            name: self.name,
            email: self.email,
            gh_avatar: self.gh_avatar,
            is_active: self.is_active,
            account_lock_reason: self.account_lock_reason,
            account_lock_until: self.account_lock_until,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        })
        .exec(db)
        .await?;

        Ok(user)
    }
}

/// Builder for ApiToken models
pub struct ApiTokenBuilder {
    user_id: u64,
    name: String,
    resource_scopes: Option<Vec<ResourceScope>>,
    action_scopes: Option<Vec<ActionScope>>,
    expired_at: Option<jiff::Timestamp>,
}

impl ApiTokenBuilder {
    pub fn new(user_id: u64, name: &str) -> Self {
        Self {
            user_id,
            name: name.to_string(),
            resource_scopes: None,
            action_scopes: None,
            expired_at: None,
        }
    }

    pub fn resource_scopes(mut self, scopes: Vec<ResourceScope>) -> Self {
        self.resource_scopes = Some(scopes);
        self
    }

    pub fn action_scopes(mut self, scopes: Vec<ActionScope>) -> Self {
        self.action_scopes = Some(scopes);
        self
    }

    pub fn expired_at(mut self, expired_at: jiff::Timestamp) -> Self {
        self.expired_at = Some(expired_at);
        self
    }

    /// Build and insert the token into the database
    pub async fn build(self, db: &mut toasty::Db) -> anyhow::Result<(ApiToken, String)> {
        let plain_token = PlainToken::generate();
        let hashed_token = plain_token.hashed();
        let token_bytes = hashed_token.as_bytes().to_vec();

        let resource_scopes = self
            .resource_scopes
            .map(|s| serde_json::to_string(&s).unwrap());
        let action_scopes = self
            .action_scopes
            .map(|s| serde_json::to_string(&s).unwrap());

        let api_token = toasty::create!(ApiToken {
            user_id: self.user_id,
            name: self.name,
            token: token_bytes,
            created_at: jiff::Timestamp::now(),
            last_used_at: None,
            revoked: false,
            resource_scopes,
            action_scopes,
            expired_at: self.expired_at,
        })
        .exec(db)
        .await?;

        Ok((api_token, plain_token.expose_secret().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_builder_defaults() {
        let builder = UserBuilder::new("test_user");
        assert_eq!(builder.gh_login, "test_user");
        assert!(builder.is_active);
        assert!(builder.name.is_none());
    }

    #[test]
    fn test_user_builder_chainable() {
        let builder = UserBuilder::new("test_user")
            .name("Test User")
            .email("test@example.com")
            .inactive();

        assert_eq!(builder.name, Some("Test User".to_string()));
        assert_eq!(builder.email, Some("test@example.com".to_string()));
        assert!(!builder.is_active);
    }

    #[test]
    fn test_api_token_builder_defaults() {
        let builder = ApiTokenBuilder::new(123, "test_token");
        assert_eq!(builder.user_id, 123);
        assert_eq!(builder.name, "test_token");
        assert!(builder.resource_scopes.is_none());
    }

    #[test]
    fn test_api_token_builder_chainable() {
        let builder = ApiTokenBuilder::new(123, "test_token")
            .resource_scopes(vec![ResourceScope::new("test*".to_string()).unwrap()])
            .action_scopes(vec![ActionScope::Read]);

        assert!(builder.resource_scopes.is_some());
        assert!(builder.action_scopes.is_some());
    }
}

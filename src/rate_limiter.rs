//! Database-backed rate limiter using token bucket algorithm
//!
//! This rate limiter persists bucket state in the configured database (Toasty/SQLite
//! by default, PostgreSQL in production). Each request consumes one token from the
//! bucket identified by the action and the bucket key (authenticated user ID or IP
//! address). Tokens refill over time at the configured rate.
//!
//! # Important
//!
//! This is a single-database token bucket implementation. It is persistent across
//! restarts and shared by all instances connecting to the same database. Under very
//! high concurrency with SQLite, the database may lock; for highly concurrent or
//! multi-instance production deployments, consider a dedicated rate-limiting service
//! (e.g. Redis-backed) on top of this schema.
//!
//! # Example Usage
//!
//! ```ignore
//! use axum_kickoff::rate_limiter::{RateLimiter, LimitedAction, RateLimiterConfig};
//! use std::time::Duration;
//! use std::collections::HashMap;
//!
//! # async fn example() {
//! let mut config = HashMap::new();
//! config.insert(
//!     LimitedAction::ApiRequest,
//!     RateLimiterConfig {
//!         rate: Duration::from_secs(1),
//!         burst: 10,
//!     },
//! );
//!
//! let rate_limiter = RateLimiter::new(config, database);
//!
//! match rate_limiter.check_rate_limit("127.0.0.1", LimitedAction::ApiRequest).await {
//!     Ok(()) => { /* allow request */ },
//!     Err(e) => { /* return 429 Too Many Requests */ },
//! }
//! # }
//! ```

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use crate::db::Database;
use crate::models::RateLimitBucket;

/// Actions that can be rate limited
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitedAction {
    /// General API requests
    ApiRequest,
    /// Login/authentication attempts
    LoginAttempt,
    /// Password reset requests
    PasswordReset,
    /// File upload operations
    FileUpload,
    /// Form submissions (contact forms, etc.)
    FormSubmission,
    /// OAuth authorize requests
    OAuthAuthorize,
    /// OAuth callback requests
    OAuthCallback,
    /// API token creation
    TokenCreation,
}

impl LimitedAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            LimitedAction::ApiRequest => "api_request",
            LimitedAction::LoginAttempt => "login_attempt",
            LimitedAction::PasswordReset => "password_reset",
            LimitedAction::FileUpload => "file_upload",
            LimitedAction::FormSubmission => "form_submission",
            LimitedAction::OAuthAuthorize => "oauth_authorize",
            LimitedAction::OAuthCallback => "oauth_callback",
            LimitedAction::TokenCreation => "token_creation",
        }
    }

    pub fn default_rate_seconds(&self) -> u64 {
        match self {
            LimitedAction::ApiRequest => 1,
            LimitedAction::LoginAttempt => 5,
            LimitedAction::PasswordReset => 60,
            LimitedAction::FileUpload => 10,
            LimitedAction::FormSubmission => 30,
            LimitedAction::OAuthAuthorize => 5,
            LimitedAction::OAuthCallback => 5,
            LimitedAction::TokenCreation => 10,
        }
    }

    pub fn default_burst(&self) -> i32 {
        match self {
            LimitedAction::ApiRequest => 10,
            LimitedAction::LoginAttempt => 5,
            LimitedAction::PasswordReset => 3,
            LimitedAction::FileUpload => 5,
            LimitedAction::FormSubmission => 10,
            LimitedAction::OAuthAuthorize => 5,
            LimitedAction::OAuthCallback => 5,
            LimitedAction::TokenCreation => 3,
        }
    }

    pub fn env_var_key(&self) -> &'static str {
        match self {
            LimitedAction::ApiRequest => "API_REQUEST",
            LimitedAction::LoginAttempt => "LOGIN_ATTEMPT",
            LimitedAction::PasswordReset => "PASSWORD_RESET",
            LimitedAction::FileUpload => "FILE_UPLOAD",
            LimitedAction::FormSubmission => "FORM_SUBMISSION",
            LimitedAction::OAuthAuthorize => "OAUTH_AUTHORIZE",
            LimitedAction::OAuthCallback => "OAUTH_CALLBACK",
            LimitedAction::TokenCreation => "TOKEN_CREATION",
        }
    }

    pub fn error_message(&self) -> &'static str {
        match self {
            LimitedAction::ApiRequest => "Too many API requests. Please slow down.",
            LimitedAction::LoginAttempt => {
                "Too many login attempts. Please wait before trying again."
            }
            LimitedAction::PasswordReset => {
                "Too many password reset requests. Please wait before trying again."
            }
            LimitedAction::FileUpload => "Too many file uploads. Please wait before trying again.",
            LimitedAction::FormSubmission => {
                "Too many form submissions. Please wait before trying again."
            }
            LimitedAction::OAuthAuthorize => {
                "Too many OAuth authorization requests. Please wait before trying again."
            }
            LimitedAction::OAuthCallback => {
                "Too many OAuth callback requests. Please wait before trying again."
            }
            LimitedAction::TokenCreation => {
                "Too many token creation requests. Please wait before trying again."
            }
        }
    }

    pub const VARIANTS: [LimitedAction; 8] = [
        LimitedAction::ApiRequest,
        LimitedAction::LoginAttempt,
        LimitedAction::PasswordReset,
        LimitedAction::FileUpload,
        LimitedAction::FormSubmission,
        LimitedAction::OAuthAuthorize,
        LimitedAction::OAuthCallback,
        LimitedAction::TokenCreation,
    ];
}

#[derive(Debug, Clone, Copy)]
pub struct RateLimiterConfig {
    pub rate: Duration,
    pub burst: i32,
}

/// Database-backed rate limiter using token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    config: HashMap<LimitedAction, RateLimiterConfig>,
    database: Database,
}

impl RateLimiter {
    pub fn new(config: HashMap<LimitedAction, RateLimiterConfig>, database: Database) -> Self {
        Self { config, database }
    }

    /// Check if an action is allowed for a given key (e.g. IP address or user ID)
    pub async fn check_rate_limit(
        &self,
        bucket_id: &str,
        action: LimitedAction,
    ) -> Result<(), RateLimitError> {
        let config = self.config_for_action(action);
        let bucket_key = format!("{}:{}", action.as_str(), bucket_id);

        let mut db = self.database.db_clone();

        // Try to find an existing bucket
        let existing = RateLimitBucket::filter(
            RateLimitBucket::fields()
                .bucket_key()
                .eq(bucket_key.clone()),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| RateLimitError {
            action,
            retry_after: config.rate,
            source: Some(e),
        })?;

        let now = jiff::Timestamp::now();

        if let Some(mut bucket) = existing {
            let elapsed = now.as_nanosecond() - bucket.last_refill.as_nanosecond();
            let tokens_to_add =
                (elapsed as f64 / (config.rate.as_secs_f64() * 1_000_000_000.0)).floor() as i32;
            let new_tokens = (bucket.tokens + tokens_to_add).min(config.burst);
            let new_last_refill = if tokens_to_add > 0 {
                now
            } else {
                bucket.last_refill
            };

            if new_tokens > 0 {
                let new_tokens = new_tokens - 1;

                bucket.tokens = new_tokens;
                bucket.last_refill = new_last_refill;

                toasty::update!(bucket {
                    tokens: new_tokens,
                    last_refill: new_last_refill,
                })
                .exec(&mut db)
                .await
                .map_err(|e| RateLimitError {
                    action,
                    retry_after: config.rate,
                    source: Some(e),
                })?;

                Ok(())
            } else {
                Err(RateLimitError {
                    action,
                    retry_after: config.rate,
                    source: None,
                })
            }
        } else {
            // New bucket: create it with one token already consumed.
            let tokens = config.burst.saturating_sub(1);
            let _ = toasty::create!(RateLimitBucket {
                bucket_key,
                action: action.as_str().to_string(),
                bucket_id: bucket_id.to_string(),
                tokens,
                last_refill: now,
            })
            .exec(&mut db)
            .await;

            Ok(())
        }
    }

    /// Check rate limit by IP address
    pub async fn check_by_ip(
        &self,
        ip: IpAddr,
        action: LimitedAction,
    ) -> Result<(), RateLimitError> {
        self.check_rate_limit(&ip.to_string(), action).await
    }

    /// Clear all rate limit buckets (useful for testing)
    pub async fn clear_all(&self) -> Result<(), toasty::Error> {
        let mut db = self.database.db_clone();
        RateLimitBucket::filter(RateLimitBucket::fields().bucket_key().ne(String::new()))
            .delete()
            .exec(&mut db)
            .await?;
        Ok(())
    }

    /// Clear rate limit bucket for a specific key
    pub async fn clear_key(&self, key: &str, action: LimitedAction) -> Result<(), toasty::Error> {
        let bucket_key = format!("{}:{}", action.as_str(), key);
        let mut db = self.database.db_clone();
        RateLimitBucket::filter(RateLimitBucket::fields().bucket_key().eq(bucket_key))
            .delete()
            .exec(&mut db)
            .await?;
        Ok(())
    }

    fn config_for_action(&self, action: LimitedAction) -> RateLimiterConfig {
        self.config
            .get(&action)
            .copied()
            .unwrap_or_else(|| RateLimiterConfig {
                rate: Duration::from_secs(action.default_rate_seconds()),
                burst: action.default_burst(),
            })
    }
}

#[derive(Debug)]
pub struct RateLimitError {
    pub action: LimitedAction,
    pub retry_after: Duration,
    source: Option<toasty::Error>,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.action.error_message())
    }
}

impl std::error::Error for RateLimitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e as _)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DatabaseConfig;
    use crate::db::Database;
    use tempfile::NamedTempFile;

    async fn test_database() -> (NamedTempFile, Database) {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());
        let database = Database::from_config(&DatabaseConfig { url: db_url.into() })
            .await
            .expect("Failed to create test database");
        database
            .migrate()
            .await
            .expect("Failed to apply test database migrations");
        (db_file, database)
    }

    #[tokio::test]
    async fn test_basic_rate_limiting() {
        let mut config = HashMap::new();
        config.insert(
            LimitedAction::ApiRequest,
            RateLimiterConfig {
                rate: Duration::from_millis(100),
                burst: 5,
            },
        );

        let (_db_file, database) = test_database().await;
        let rate_limiter = RateLimiter::new(config, database);
        let ip = "127.0.0.1".parse().unwrap();

        for _ in 0..5 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }

        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::ApiRequest)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_token_refill() {
        let mut config = HashMap::new();
        config.insert(
            LimitedAction::ApiRequest,
            RateLimiterConfig {
                rate: Duration::from_millis(100),
                burst: 5,
            },
        );

        let (_db_file, database) = test_database().await;
        let rate_limiter = RateLimiter::new(config, database);
        let ip = "127.0.0.1".parse().unwrap();

        for _ in 0..5 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }
        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::ApiRequest)
            .await
            .is_err());

        tokio::time::sleep(Duration::from_millis(150)).await;

        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::ApiRequest)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_different_keys_independent() {
        let mut config = HashMap::new();
        config.insert(
            LimitedAction::ApiRequest,
            RateLimiterConfig {
                rate: Duration::from_secs(1),
                burst: 2,
            },
        );

        let (_db_file, database) = test_database().await;
        let rate_limiter = RateLimiter::new(config, database);
        let ip1 = "127.0.0.1".parse().unwrap();
        let ip2 = "127.0.0.2".parse().unwrap();

        for _ in 0..2 {
            assert!(rate_limiter
                .check_by_ip(ip1, LimitedAction::ApiRequest)
                .await
                .is_ok());
            assert!(rate_limiter
                .check_by_ip(ip2, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }

        assert!(rate_limiter
            .check_by_ip(ip1, LimitedAction::ApiRequest)
            .await
            .is_err());
        assert!(rate_limiter
            .check_by_ip(ip2, LimitedAction::ApiRequest)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_different_actions_independent() {
        let mut config = HashMap::new();
        config.insert(
            LimitedAction::ApiRequest,
            RateLimiterConfig {
                rate: Duration::from_secs(1),
                burst: 2,
            },
        );
        config.insert(
            LimitedAction::LoginAttempt,
            RateLimiterConfig {
                rate: Duration::from_secs(1),
                burst: 5,
            },
        );

        let (_db_file, database) = test_database().await;
        let rate_limiter = RateLimiter::new(config, database);
        let ip = "127.0.0.1".parse().unwrap();

        for _ in 0..2 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }
        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::ApiRequest)
            .await
            .is_err());

        for _ in 0..5 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::LoginAttempt)
                .await
                .is_ok());
        }
        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::LoginAttempt)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_clear_key() {
        let mut config = HashMap::new();
        config.insert(
            LimitedAction::ApiRequest,
            RateLimiterConfig {
                rate: Duration::from_secs(1),
                burst: 2,
            },
        );

        let (_db_file, database) = test_database().await;
        let rate_limiter = RateLimiter::new(config, database);
        let ip = "127.0.0.1".parse().unwrap();

        for _ in 0..2 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }
        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::ApiRequest)
            .await
            .is_err());

        rate_limiter
            .clear_key(&ip.to_string(), LimitedAction::ApiRequest)
            .await
            .expect("clear_key should succeed");

        for _ in 0..2 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }
    }

    #[tokio::test]
    async fn test_default_config() {
        let (_db_file, database) = test_database().await;
        let rate_limiter = RateLimiter::new(HashMap::new(), database);
        let ip = "127.0.0.1".parse().unwrap();

        for _ in 0..10 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }
        assert!(rate_limiter
            .check_by_ip(ip, LimitedAction::ApiRequest)
            .await
            .is_err());
    }
}

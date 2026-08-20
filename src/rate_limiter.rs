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
//! restarts and shared by all instances connecting to the same database. The token
//! take is performed as a single atomic SQL statement (upsert for SQLite and
//! PostgreSQL) so concurrent requests for the same bucket cannot overspend.
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
use std::sync::Arc;
use std::time::Duration;

use toasty::db::{Capability, SqlPlaceholder};
use toasty::sql;
use toasty::stmt::Value;

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
    clock: Arc<dyn Fn() -> jiff::Timestamp + Send + Sync>,
}

impl RateLimiter {
    pub fn new(config: HashMap<LimitedAction, RateLimiterConfig>, database: Database) -> Self {
        Self::with_clock(
            config,
            database,
            Arc::new(jiff::Timestamp::now) as Arc<dyn Fn() -> jiff::Timestamp + Send + Sync>,
        )
    }

    fn with_clock(
        config: HashMap<LimitedAction, RateLimiterConfig>,
        database: Database,
        clock: Arc<dyn Fn() -> jiff::Timestamp + Send + Sync>,
    ) -> Self {
        Self {
            config,
            database,
            clock,
        }
    }

    /// Check if an action is allowed for a given key (e.g. IP address or user ID)
    pub async fn check_rate_limit(
        &self,
        bucket_id: &str,
        action: LimitedAction,
    ) -> Result<(), RateLimitError> {
        let config = self.config_for_action(action);
        let bucket_key = format!("{}:{}", action.as_str(), bucket_id);

        let now = (self.clock)();
        let initial_tokens = config.burst.saturating_sub(1);
        let rate = config.rate.as_secs_f64();

        let mut db = self.database.db_clone();
        let capability = db.capability();

        let sql = build_take_sql(capability);

        let query = sql::query(sql)
            .bind(&bucket_key)
            .bind(action.as_str())
            .bind(bucket_id)
            .bind(initial_tokens)
            .bind(timestamp_value(capability.sql_placeholder, now))
            .bind(rate)
            .bind(config.burst);

        let rows = query.exec(&mut db).await.map_err(|e| RateLimitError {
            action,
            retry_after: config.rate,
            source: Some(e),
        })?;

        let tokens = parse_returned_tokens(rows).ok_or_else(|| RateLimitError {
            action,
            retry_after: config.rate,
            source: None,
        })?;

        if tokens < 0 {
            Err(RateLimitError {
                action,
                retry_after: config.rate,
                source: None,
            })
        } else {
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

fn build_take_sql(capability: &Capability) -> &'static str {
    match capability.sql_placeholder {
        Some(SqlPlaceholder::DollarNumber) => POSTGRES_TAKE_SQL,
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            SQLITE_TAKE_SQL
        }
        None => panic!("raw SQL rate limiting requires a SQL backend"),
    }
}

pub(crate) fn timestamp_value(placeholder: Option<SqlPlaceholder>, ts: jiff::Timestamp) -> Value {
    match placeholder {
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            Value::String(timestamp_storage_text(ts))
        }
        _ => Value::Timestamp(ts),
    }
}

pub(crate) fn timestamp_storage_text(ts: jiff::Timestamp) -> String {
    let rounded = ts
        .round(
            jiff::TimestampRound::new()
                .smallest(jiff::Unit::Microsecond)
                .mode(jiff::RoundMode::Trunc),
        )
        .unwrap_or(ts);
    format!("{:.6}", rounded)
}

const POSTGRES_TAKE_SQL: &str = r#"
    INSERT INTO rate_limit_buckets AS b (bucket_key, action, bucket_id, tokens, last_refill)
    VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT (bucket_key) DO UPDATE
    SET tokens = CASE
        WHEN GREATEST(0, b.tokens) + FLOOR(EXTRACT(EPOCH FROM (EXCLUDED.last_refill - b.last_refill)) / $6)::integer >= 1
        THEN LEAST(GREATEST(0, b.tokens) + FLOOR(EXTRACT(EPOCH FROM (EXCLUDED.last_refill - b.last_refill)) / $6)::integer, $7) - 1
        ELSE -1
    END,
    last_refill = CASE
        WHEN GREATEST(0, b.tokens) + FLOOR(EXTRACT(EPOCH FROM (EXCLUDED.last_refill - b.last_refill)) / $6)::integer >= 1
        THEN EXCLUDED.last_refill
        ELSE b.last_refill
    END
    RETURNING tokens
"#;

const SQLITE_TAKE_SQL: &str = r#"
    INSERT INTO rate_limit_buckets AS b (bucket_key, action, bucket_id, tokens, last_refill)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT (bucket_key) DO UPDATE
    SET tokens = CASE
        WHEN MAX(0, b.tokens) + CAST((((julianday(EXCLUDED.last_refill) - julianday(b.last_refill)) * 86400) / ?6) AS INTEGER) >= 1
        THEN MIN(MAX(0, b.tokens) + CAST((((julianday(EXCLUDED.last_refill) - julianday(b.last_refill)) * 86400) / ?6) AS INTEGER), ?7) - 1
        ELSE -1
    END,
    last_refill = CASE
        WHEN MAX(0, b.tokens) + CAST((((julianday(EXCLUDED.last_refill) - julianday(b.last_refill)) * 86400) / ?6) AS INTEGER) >= 1
        THEN EXCLUDED.last_refill
        ELSE b.last_refill
    END
    RETURNING tokens
"#;

fn parse_returned_tokens(rows: Vec<Value>) -> Option<i32> {
    let record = rows.into_iter().next()?.as_record()?.clone();
    record.fields.into_iter().next()?.to_i32()
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
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    #[derive(Clone)]
    struct TestClock {
        current: Arc<Mutex<jiff::Timestamp>>,
    }

    impl TestClock {
        fn new() -> Self {
            Self {
                current: Arc::new(Mutex::new(jiff::Timestamp::now())),
            }
        }

        fn now(&self) -> jiff::Timestamp {
            *self.current.lock().unwrap()
        }

        fn advance(&self, duration: Duration) {
            let mut current = self.current.lock().unwrap();
            *current = current.checked_add(duration).expect("valid duration");
        }
    }

    fn rate_limiter_with_clock(
        config: HashMap<LimitedAction, RateLimiterConfig>,
        database: Database,
        clock: &TestClock,
    ) -> RateLimiter {
        RateLimiter::with_clock(
            config,
            database,
            Arc::new({
                let clock = clock.clone();
                move || clock.now()
            }),
        )
    }

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
                rate: Duration::from_millis(1500),
                burst: 5,
            },
        );

        let (_db_file, database) = test_database().await;
        let clock = TestClock::new();
        let rate_limiter = rate_limiter_with_clock(config, database, &clock);
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
                rate: Duration::from_millis(1500),
                burst: 5,
            },
        );

        let (_db_file, database) = test_database().await;
        let clock = TestClock::new();
        let rate_limiter = rate_limiter_with_clock(config, database, &clock);
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

        clock.advance(Duration::from_millis(1600));

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
        let clock = TestClock::new();
        let rate_limiter = rate_limiter_with_clock(config, database, &clock);
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
        let clock = TestClock::new();
        let rate_limiter = rate_limiter_with_clock(config, database, &clock);
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
        let clock = TestClock::new();
        let rate_limiter = rate_limiter_with_clock(config, database, &clock);
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
    async fn test_concurrent_rate_limiting_is_atomic() {
        let mut config = HashMap::new();
        config.insert(
            LimitedAction::ApiRequest,
            RateLimiterConfig {
                rate: Duration::from_secs(1),
                burst: 10,
            },
        );

        let (_db_file, database) = test_database().await;
        let clock = TestClock::new();
        let rate_limiter = Arc::new(rate_limiter_with_clock(config, database, &clock));
        let ip = "127.0.0.1".parse().unwrap();

        let mut handles = Vec::new();
        for _ in 0..20 {
            let rl = rate_limiter.clone();
            handles.push(tokio::spawn(async move {
                rl.check_by_ip(ip, LimitedAction::ApiRequest).await.is_ok()
            }));
        }

        let mut allowed = 0;
        for handle in handles {
            if handle.await.unwrap() {
                allowed += 1;
            }
        }

        assert!(
            allowed <= 10,
            "concurrent calls allowed {allowed}, but burst is 10"
        );
    }

    #[tokio::test]
    async fn test_default_config() {
        let (_db_file, database) = test_database().await;
        let clock = TestClock::new();
        let rate_limiter = rate_limiter_with_clock(HashMap::new(), database, &clock);
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

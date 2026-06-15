//! In-memory rate limiter using token bucket algorithm
//!
//! This provides a simple rate limiting solution suitable for single-instance deployments.
//! For production use with persistence or distributed systems, see the upgrade documentation
//! in `docs/rate-limiting-redis-upgrade.md`.
//!
//! # What is Rate Limiting?
//!
//! Rate limiting controls how many requests a user can make in a given time period.
//! This prevents abuse, protects your server from overload, and ensures fair usage
//! among all users.
//!
//! # How It Works
//!
//! This implementation uses the **token bucket algorithm** with in-memory storage:
//! - Each user has a "bucket" of tokens stored in memory
//! - Each request consumes one token
//! - Tokens refill over time at a configured rate
//! - If the bucket is empty, requests are rejected
//! - The "burst" size is the maximum tokens a bucket can hold
//!
//! # Example Usage
//!
//! ```no_run
//! use axum_kickoff::rate_limiter::{RateLimiter, LimitedAction, RateLimiterConfig};
//! use std::time::Duration;
//! use std::collections::HashMap;
//! use std::net::IpAddr;
//!
//! # async fn example() {
//! // Configure rate limits
//! let mut config = HashMap::new();
//! config.insert(
//!     LimitedAction::ApiRequest,
//!     RateLimiterConfig {
//!         rate: Duration::from_secs(1),  // 1 token per second
//!         burst: 10,                     // max 10 tokens in bucket
//!     },
//! );
//!
//! let rate_limiter = RateLimiter::new(config);
//!
//! // Check if a request is allowed
//! let ip_address = IpAddr::from([127, 0, 0, 1]);
//! match rate_limiter.check_by_ip(ip_address, LimitedAction::ApiRequest).await {
//!     Ok(()) => { /* allow request */ },
//!     Err(e) => { /* return 429 Too Many Requests */ },
//! }
//! # }
//! ```

use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Actions that can be rate limited
///
/// These are common actions for web applications. You can add custom actions
/// by extending this enum or using string-based keys in your own implementation.
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
}

impl LimitedAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            LimitedAction::ApiRequest => "api_request",
            LimitedAction::LoginAttempt => "login_attempt",
            LimitedAction::PasswordReset => "password_reset",
            LimitedAction::FileUpload => "file_upload",
            LimitedAction::FormSubmission => "form_submission",
        }
    }

    pub fn default_rate_seconds(&self) -> u64 {
        match self {
            LimitedAction::ApiRequest => 1,      // 1 request per second
            LimitedAction::LoginAttempt => 5,    // 1 login every 5 seconds
            LimitedAction::PasswordReset => 60,  // 1 reset per minute
            LimitedAction::FileUpload => 10,     // 1 upload every 10 seconds
            LimitedAction::FormSubmission => 30, // 1 form every 30 seconds
        }
    }

    pub fn default_burst(&self) -> i32 {
        match self {
            LimitedAction::ApiRequest => 10,
            LimitedAction::LoginAttempt => 5,
            LimitedAction::PasswordReset => 3,
            LimitedAction::FileUpload => 5,
            LimitedAction::FormSubmission => 10,
        }
    }

    pub fn env_var_key(&self) -> &'static str {
        match self {
            LimitedAction::ApiRequest => "API_REQUEST",
            LimitedAction::LoginAttempt => "LOGIN_ATTEMPT",
            LimitedAction::PasswordReset => "PASSWORD_RESET",
            LimitedAction::FileUpload => "FILE_UPLOAD",
            LimitedAction::FormSubmission => "FORM_SUBMISSION",
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
        }
    }

    pub const VARIANTS: [LimitedAction; 5] = [
        LimitedAction::ApiRequest,
        LimitedAction::LoginAttempt,
        LimitedAction::PasswordReset,
        LimitedAction::FileUpload,
        LimitedAction::FormSubmission,
    ];
}

#[derive(Debug, Clone, Copy)]
pub struct RateLimiterConfig {
    pub rate: Duration,
    pub burst: i32,
}

/// Token bucket state for a single key
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: i32,
    last_refill: Instant,
}

/// In-memory rate limiter using token bucket algorithm
pub struct RateLimiter {
    config: HashMap<LimitedAction, RateLimiterConfig>,
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
}

impl RateLimiter {
    pub fn new(config: HashMap<LimitedAction, RateLimiterConfig>) -> Self {
        Self {
            config,
            buckets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if an action is allowed for a given key (e.g., IP address or user ID)
    pub async fn check_rate_limit(
        &self,
        key: &str,
        action: LimitedAction,
    ) -> Result<(), RateLimitError> {
        let config = self.config_for_action(action);
        let bucket_key = format!("{}:{}", action.as_str(), key);

        let mut buckets = self.buckets.write();
        let now = Instant::now();

        let bucket = buckets
            .entry(bucket_key.clone())
            .or_insert_with(|| TokenBucket {
                tokens: config.burst,
                last_refill: now,
            });

        // Calculate tokens to add based on time elapsed
        let elapsed = now.duration_since(bucket.last_refill);
        let refill_rate_secs = config.rate.as_secs_f64();
        let tokens_to_add = (elapsed.as_secs_f64() / refill_rate_secs).floor() as i32;

        if tokens_to_add > 0 {
            bucket.tokens = (bucket.tokens + tokens_to_add).min(config.burst);
            bucket.last_refill = now;
        }

        // Check if we have tokens available
        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            Ok(())
        } else {
            Err(RateLimitError {
                action,
                retry_after: config.rate,
            })
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
    pub fn clear_all(&self) {
        self.buckets.write().clear();
    }

    /// Clear rate limit bucket for a specific key
    pub fn clear_key(&self, key: &str, action: LimitedAction) {
        let bucket_key = format!("{}:{}", action.as_str(), key);
        self.buckets.write().remove(&bucket_key);
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
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.action.error_message())
    }
}

impl std::error::Error for RateLimitError {}

#[cfg(test)]
mod tests {
    use super::*;

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

        let rate_limiter = RateLimiter::new(config);
        let ip = "127.0.0.1".parse().unwrap();

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }

        // 6th request should be rate limited
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

        let rate_limiter = RateLimiter::new(config);
        let ip = "127.0.0.1".parse().unwrap();

        // Use all tokens
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

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should allow another request
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

        let rate_limiter = RateLimiter::new(config);
        let ip1 = "127.0.0.1".parse().unwrap();
        let ip2 = "127.0.0.2".parse().unwrap();

        // Each IP should have independent limits
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

        // Both should be rate limited now
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

        let rate_limiter = RateLimiter::new(config);
        let ip = "127.0.0.1".parse().unwrap();

        // API requests should be limited to 2
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

        // Login attempts should still have 5 available
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

        let rate_limiter = RateLimiter::new(config);
        let ip = "127.0.0.1".parse().unwrap();

        // Use all tokens
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

        // Clear the bucket
        rate_limiter.clear_key(&ip.to_string(), LimitedAction::ApiRequest);

        // Should allow requests again
        for _ in 0..2 {
            assert!(rate_limiter
                .check_by_ip(ip, LimitedAction::ApiRequest)
                .await
                .is_ok());
        }
    }

    #[tokio::test]
    async fn test_default_config() {
        let rate_limiter = RateLimiter::new(HashMap::new());
        let ip = "127.0.0.1".parse().unwrap();

        // Should use default config for ApiRequest (burst: 10)
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

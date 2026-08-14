//! Persisted token-bucket state for the global rate limiter.

use toasty::Model;

/// A persisted token bucket for a specific action + bucket identifier.
///
/// `bucket_key` is the composite primary key (`{action}:{bucket_id}`) so that
/// concurrent requests for the same bucket can be handled with a unique key
/// constraint in the database.
#[derive(Debug, Model)]
pub struct RateLimitBucket {
    #[key]
    pub bucket_key: String,
    pub action: String,
    pub bucket_id: String,
    pub tokens: i32,
    pub last_refill: jiff::Timestamp,
}

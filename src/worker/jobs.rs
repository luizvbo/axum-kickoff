//! Built-in background jobs and the [`Job`] trait.

use std::future::Future;
use std::pin::Pin;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use toasty::db::Capability;
use toasty::sql;

use crate::app::App;
use crate::db::Database;
use crate::rate_limiter::timestamp_value;

/// Trait that all background jobs must implement.
///
/// `Job` is intentionally object-safe and bounded by `DeserializeOwned` so that
/// the worker can deserialize a JSON payload from the `background_jobs` table
/// and dispatch it to the correct handler.
pub trait Job: serde::de::DeserializeOwned + Send + Sync + 'static {
    /// Unique name for this job type; stored in `background_jobs.job_type`.
    const NAME: &'static str;

    /// Execute the job. Implementations **must** be idempotent: the worker
    /// claims jobs with a lease, so a crashed or slow worker's job can be
    /// reclaimed and run again by another worker in addition to being
    /// retried on failure.
    fn run<'a>(
        &'a self,
        app: &'a App,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
}

/// Deletes `rate_limit_buckets` rows whose `last_refill` is older than the
/// configured number of days. This is an idempotent job: running it twice with
/// the same `max_age_days` simply deletes nothing on the second run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupJob {
    pub max_age_days: u64,
}

impl CleanupJob {
    pub fn new(max_age_days: u64) -> Self {
        Self { max_age_days }
    }
}

impl Job for CleanupJob {
    const NAME: &'static str = "cleanup";

    fn run<'a>(
        &'a self,
        app: &'a App,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(
            async move { cleanup_old_rate_limit_buckets(&app.database, self.max_age_days).await },
        )
    }
}

async fn cleanup_old_rate_limit_buckets(
    database: &Database,
    max_age_days: u64,
) -> anyhow::Result<()> {
    // `max_age_days as i64 * 86400` wraps or overflows for pathological
    // values — a wrapped negative turns the `checked_sub` into an addition,
    // and the old `unwrap_or(now)` fallback then made the cutoff *now*,
    // deleting every bucket instead of none. Saturate the multiplication and
    // pin the cutoff to the UNIX epoch so an unrepresentable retention window
    // deletes nothing.
    let max_age = jiff::SignedDuration::from_secs(
        i64::try_from(max_age_days)
            .unwrap_or(i64::MAX)
            .saturating_mul(86400),
    );
    let cutoff = jiff::Timestamp::now()
        .checked_sub(max_age)
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH);

    let mut db = database.db_clone();
    let cap = db.capability();

    let sql = build_delete_sql(cap);

    sql::statement(sql)
        .bind(timestamp_value(cap.sql_placeholder, cutoff))
        .exec(&mut db)
        .await
        .context("Failed to clean up old rate limit buckets")?;

    Ok(())
}

fn build_delete_sql(cap: &Capability) -> &'static str {
    match cap.sql_placeholder {
        Some(toasty::SqlPlaceholder::DollarNumber) => {
            "DELETE FROM rate_limit_buckets WHERE last_refill < $1"
        }
        Some(toasty::SqlPlaceholder::NumberedQuestionMark)
        | Some(toasty::SqlPlaceholder::QuestionMark) => {
            "DELETE FROM rate_limit_buckets WHERE last_refill < ?1"
        }
        None => panic!("raw SQL cleanup requires a SQL backend"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RateLimitBucket;
    use crate::tests::test_app::TestApp;

    /// A `max_age_days` that overflows `i64 * 86400` must not delete anything:
    /// before the fix the wrapped/negated duration turned the cutoff into a
    /// *future* timestamp, wiping live buckets.
    #[tokio::test]
    async fn test_cleanup_with_huge_max_age_deletes_nothing() {
        let test_app = TestApp::new().await;
        let database = test_app.state.0.database.clone();

        let mut db = database.db_clone();
        toasty::create!(RateLimitBucket {
            bucket_key: "api_request:1.2.3.4".to_string(),
            action: "api_request".to_string(),
            bucket_id: "1.2.3.4".to_string(),
            tokens: 3,
            last_refill: jiff::Timestamp::now(),
        })
        .exec(&mut db)
        .await
        .unwrap();

        cleanup_old_rate_limit_buckets(&database, u64::MAX)
            .await
            .unwrap();

        let found = RateLimitBucket::filter(
            RateLimitBucket::fields()
                .bucket_key()
                .eq("api_request:1.2.3.4".to_string()),
        )
        .first()
        .exec(&mut db)
        .await
        .unwrap();
        assert!(found.is_some(), "bucket must survive a huge max_age_days");
    }

    /// Sensible values still prune: a bucket older than `max_age_days` is
    /// deleted, a fresh one survives.
    #[tokio::test]
    async fn test_cleanup_deletes_only_stale_buckets() {
        let test_app = TestApp::new().await;
        let database = test_app.state.0.database.clone();

        let mut db = database.db_clone();
        let old = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(86400 * 10))
            .unwrap();
        toasty::create!(RateLimitBucket {
            bucket_key: "api_request:stale".to_string(),
            action: "api_request".to_string(),
            bucket_id: "stale".to_string(),
            tokens: 0,
            last_refill: old,
        })
        .exec(&mut db)
        .await
        .unwrap();
        toasty::create!(RateLimitBucket {
            bucket_key: "api_request:fresh".to_string(),
            action: "api_request".to_string(),
            bucket_id: "fresh".to_string(),
            tokens: 0,
            last_refill: jiff::Timestamp::now(),
        })
        .exec(&mut db)
        .await
        .unwrap();

        cleanup_old_rate_limit_buckets(&database, 5).await.unwrap();

        let stale = RateLimitBucket::filter(
            RateLimitBucket::fields()
                .bucket_key()
                .eq("api_request:stale".to_string()),
        )
        .first()
        .exec(&mut db)
        .await
        .unwrap();
        let fresh = RateLimitBucket::filter(
            RateLimitBucket::fields()
                .bucket_key()
                .eq("api_request:fresh".to_string()),
        )
        .first()
        .exec(&mut db)
        .await
        .unwrap();
        assert!(stale.is_none());
        assert!(fresh.is_some());
    }
}

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

    /// Execute the job. Implementations should be idempotent because they may be
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
    let max_age = jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
    let cutoff = jiff::Timestamp::now()
        .checked_sub(max_age)
        .unwrap_or(jiff::Timestamp::now());

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

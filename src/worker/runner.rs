//! Background worker runner.
//!
//! The runner polls `background_jobs` for jobs that are due, executes them using
//! the registered handler, and deletes the job on success or reschedules it on
//! failure with exponential backoff.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use toasty::db::{SqlPlaceholder, Transaction};
use toasty::sql;
use toasty::stmt::Value;
use toasty_core::stmt::ValueRecord;
use tokio::time::sleep;
use tracing::{error, info};

use crate::app::App;
use crate::rate_limiter::timestamp_value;
use crate::worker::Job;

const PG_RESERVE_SELECT_SQL: &str = r#"
    SELECT id, job_type, data, retries
    FROM background_jobs
    WHERE queue = $1 AND run_at <= $2
    ORDER BY priority DESC, run_at ASC, created_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
"#;

const PG_RESERVE_UPDATE_SQL: &str = "UPDATE background_jobs SET run_at = $1 WHERE id = $2";

const SQLITE_RESERVE_SQL: &str = r#"
    UPDATE background_jobs
    SET run_at = ?1
    WHERE id = (
        SELECT id FROM background_jobs
        WHERE queue = ?2 AND run_at <= ?3
        ORDER BY priority DESC, run_at ASC, created_at ASC
        LIMIT 1
    )
    RETURNING id, job_type, data, retries
"#;

const PG_DELETE_SQL: &str = "DELETE FROM background_jobs WHERE id = $1";
const SQLITE_DELETE_SQL: &str = "DELETE FROM background_jobs WHERE id = ?1";

const PG_RETRY_SQL: &str =
    "UPDATE background_jobs SET retries = retries + 1, run_at = $1 WHERE id = $2";
const SQLITE_RETRY_SQL: &str =
    "UPDATE background_jobs SET retries = retries + 1, run_at = ?1 WHERE id = ?2";

/// Handles polling, locking, and running of background jobs.
pub struct Runner {
    app: Arc<App>,
    handlers: HashMap<&'static str, Box<dyn JobHandler>>,
    poll_interval: Duration,
    queue: String,
}

struct ClaimedJob {
    id: u64,
    job_type: String,
    data: String,
    retries: i32,
}

impl Runner {
    /// Create a new worker runner attached to the given application.
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            handlers: HashMap::new(),
            poll_interval: Duration::from_secs(5),
            queue: "default".to_string(),
        }
    }

    /// How long to wait between polls when no job is available.
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Which queue this runner consumes.
    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.queue = queue.into();
        self
    }

    /// Register a job type that this runner can execute.
    pub fn register<J: Job>(mut self) -> Self {
        self.handlers.insert(
            J::NAME,
            Box::new(JobHandlerImpl::<J>(std::marker::PhantomData)),
        );
        self
    }

    /// Register the default built-in jobs.
    pub fn register_default_jobs(self) -> Self {
        self.register::<crate::worker::jobs::CleanupJob>()
    }

    /// Start the worker loop. Returns when a shutdown signal is received.
    pub async fn run(mut self) -> anyhow::Result<()> {
        info!(queue = %self.queue, "Background worker started");

        loop {
            tokio::select! {
                _ = shutdown_signal() => {
                    info!("Background worker shutting down");
                    return Ok(());
                }
                result = self.run_once() => match result {
                    Ok(true) => continue,
                    Ok(false) => {
                        tokio::select! {
                            _ = shutdown_signal() => {
                                info!("Background worker shutting down");
                                return Ok(());
                            }
                            _ = sleep(self.poll_interval) => {}
                        }
                    }
                    Err(e) => {
                        error!(error = ?e, "Background worker error");
                        tokio::select! {
                            _ = shutdown_signal() => {
                                info!("Background worker shutting down");
                                return Ok(());
                            }
                            _ = sleep(Duration::from_secs(1)) => {}
                        }
                    }
                }
            }
        }
    }

    /// Poll the queue once, claiming and running at most one job.
    ///
    /// Returns `Ok(true)` when a job was found and processed, and `Ok(false)`
    /// when the queue is empty.
    pub async fn run_once(&mut self) -> anyhow::Result<bool> {
        let mut db = self.app.database.db_clone();
        let placeholder = db.capability().sql_placeholder;
        let now = jiff::Timestamp::now();
        let reserved = jiff::Timestamp::MAX;

        // Reserve the next due job.
        let mut reserve_tx = if placeholder == Some(SqlPlaceholder::NumberedQuestionMark) {
            db.transaction_builder()
                .mode(toasty_core::driver::operation::TransactionMode::Immediate)
                .begin()
                .await
        } else {
            db.transaction().await
        }
        .context("Failed to start worker reservation transaction")?;

        let job = match reserve_next(&mut reserve_tx, placeholder, &self.queue, now, reserved).await
        {
            Ok(Some(job)) => job,
            Ok(None) => {
                reserve_tx
                    .commit()
                    .await
                    .context("Failed to commit empty worker transaction")?;
                return Ok(false);
            }
            Err(e) => {
                reserve_tx
                    .rollback()
                    .await
                    .context("Failed to rollback worker transaction")?;
                return Err(e);
            }
        };

        reserve_tx
            .commit()
            .await
            .context("Failed to commit worker reservation transaction")?;

        // Execute the job outside of the reservation transaction so that the
        // job is free to perform its own database writes.
        let result = if let Some(handler) = self.handlers.get(job.job_type.as_str()) {
            handler.run(&self.app, &job.data).await
        } else {
            Err(anyhow::anyhow!(
                "No handler registered for job type '{}'",
                job.job_type
            ))
        };

        // Finalize in a new transaction: delete on success, reschedule on failure.
        let mut finalize_tx = db
            .transaction()
            .await
            .context("Failed to start finalization transaction")?;

        match result {
            Ok(()) => {
                delete_job(&mut finalize_tx, placeholder, job.id)
                    .await
                    .context("Failed to delete completed job")?;
                finalize_tx
                    .commit()
                    .await
                    .context("Failed to commit worker finalization transaction")?;
                info!(job_id = %job.id, job_type = %job.job_type, "Job completed");
            }
            Err(e) => {
                let next_run = compute_retry_time(now, job.retries);
                update_job(&mut finalize_tx, placeholder, job.id, next_run)
                    .await
                    .context("Failed to reschedule failed job")?;
                finalize_tx
                    .commit()
                    .await
                    .context("Failed to commit worker finalization transaction")?;
                info!(
                    job_id = %job.id,
                    job_type = %job.job_type,
                    error = ?e,
                    "Job failed, rescheduled"
                );
            }
        }

        Ok(true)
    }
}

trait JobHandler: Send + Sync {
    fn run<'a>(
        &'a self,
        app: &'a App,
        data: &'a str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
}

struct JobHandlerImpl<J>(std::marker::PhantomData<J>);

impl<J: Job> JobHandler for JobHandlerImpl<J> {
    fn run<'a>(
        &'a self,
        app: &'a App,
        data: &'a str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let job: J = serde_json::from_str(data)
                .with_context(|| format!("Failed to deserialize job payload for {}", J::NAME))?;
            job.run(app).await
        })
    }
}

async fn reserve_next(
    tx: &mut Transaction<'_>,
    placeholder: Option<SqlPlaceholder>,
    queue: &str,
    now: jiff::Timestamp,
    reserved: jiff::Timestamp,
) -> anyhow::Result<Option<ClaimedJob>> {
    let reserved_value = timestamp_value(placeholder, reserved);
    let now_value = timestamp_value(placeholder, now);

    match placeholder {
        Some(SqlPlaceholder::DollarNumber) => {
            let rows = sql::query(PG_RESERVE_SELECT_SQL)
                .bind(queue)
                .bind(now_value)
                .exec(tx)
                .await
                .context("Failed to select next job")?;

            let record = match rows.into_iter().next() {
                None => return Ok(None),
                Some(Value::Record(record)) => record,
                Some(other) => anyhow::bail!("Expected record row, got {other:?}"),
            };

            let job = parse_record(record)?;

            sql::statement(PG_RESERVE_UPDATE_SQL)
                .bind(reserved_value)
                .bind(job.id)
                .exec(tx)
                .await
                .context("Failed to reserve next job")?;

            Ok(Some(job))
        }
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            let rows = sql::query(SQLITE_RESERVE_SQL)
                .bind(reserved_value)
                .bind(queue)
                .bind(now_value)
                .exec(tx)
                .await
                .context("Failed to reserve next job")?;

            let record = match rows.into_iter().next() {
                None => return Ok(None),
                Some(Value::Record(record)) => record,
                Some(other) => anyhow::bail!("Expected record row, got {other:?}"),
            };

            Ok(Some(parse_record(record)?))
        }
        None => anyhow::bail!("raw SQL worker requires a SQL backend"),
    }
}

fn parse_record(record: ValueRecord) -> anyhow::Result<ClaimedJob> {
    let mut fields = record.fields.into_iter();
    let id = fields
        .next()
        .and_then(|v| v.to_u64())
        .ok_or_else(|| anyhow::anyhow!("Expected id field"))?;
    let job_type = fields
        .next()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Expected job_type field"))?;
    let data = fields
        .next()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Expected data field"))?;
    let retries = fields
        .next()
        .and_then(|v| v.to_i32())
        .ok_or_else(|| anyhow::anyhow!("Expected retries field"))?;

    Ok(ClaimedJob {
        id,
        job_type,
        data,
        retries,
    })
}

async fn delete_job(
    tx: &mut Transaction<'_>,
    placeholder: Option<SqlPlaceholder>,
    id: u64,
) -> anyhow::Result<()> {
    let sql = match placeholder {
        Some(SqlPlaceholder::DollarNumber) => PG_DELETE_SQL,
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            SQLITE_DELETE_SQL
        }
        None => anyhow::bail!("raw SQL worker requires a SQL backend"),
    };

    sql::statement(sql)
        .bind(id)
        .exec(tx)
        .await
        .context("Failed to delete completed job")?;
    Ok(())
}

async fn update_job(
    tx: &mut Transaction<'_>,
    placeholder: Option<SqlPlaceholder>,
    id: u64,
    next_run: jiff::Timestamp,
) -> anyhow::Result<()> {
    let sql = match placeholder {
        Some(SqlPlaceholder::DollarNumber) => PG_RETRY_SQL,
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            SQLITE_RETRY_SQL
        }
        None => anyhow::bail!("raw SQL worker requires a SQL backend"),
    };

    sql::statement(sql)
        .bind(timestamp_value(placeholder, next_run))
        .bind(id)
        .exec(tx)
        .await
        .context("Failed to reschedule failed job")?;
    Ok(())
}

fn compute_retry_time(now: jiff::Timestamp, retries: i32) -> jiff::Timestamp {
    let delay = 2_i64.checked_pow(retries as u32).unwrap_or(i64::MAX);
    now.checked_add(jiff::SignedDuration::from_secs(delay))
        .unwrap_or(now)
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).expect("failed to create SIGTERM listener");
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to create SIGINT listener");

    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BackgroundJob, RateLimitBucket};
    use crate::tests::test_app::TestApp;
    use crate::worker::CleanupJob;

    #[tokio::test]
    async fn test_run_once_processes_and_deletes_job() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // Insert a rate limit bucket older than one day.
        let old = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(86400 * 2))
            .unwrap();
        let mut db = app.database.db_clone();
        toasty::create!(RateLimitBucket {
            bucket_key: "test:old".to_string(),
            action: "api_request".to_string(),
            bucket_id: "old".to_string(),
            tokens: 0,
            last_refill: old,
        })
        .exec(&mut db)
        .await
        .unwrap();

        app.enqueue_job(CleanupJob::new(1)).await.unwrap();

        let mut runner = Runner::new(app.clone()).register_default_jobs();
        assert!(runner.run_once().await.unwrap());

        // The cleanup job should have deleted the old bucket.
        let found = RateLimitBucket::filter(
            RateLimitBucket::fields()
                .bucket_key()
                .eq("test:old".to_string()),
        )
        .first()
        .exec(&mut db)
        .await
        .unwrap();
        assert!(found.is_none());

        // The job should also have been deleted from the queue.
        let remaining =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("cleanup".to_string()))
                .first()
                .exec(&mut db)
                .await
                .unwrap();
        assert!(remaining.is_none());

        // With the queue empty, run_once returns false.
        assert!(!runner.run_once().await.unwrap());
    }

    #[tokio::test]
    async fn test_failed_job_is_rescheduled_with_backoff() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // The "unknown" job type has no registered handler, so it will fail and
        // be rescheduled.
        let mut db = app.database.db_clone();
        toasty::create!(BackgroundJob {
            queue: "default".to_string(),
            job_type: "unknown".to_string(),
            data: "{}".to_string(),
            retries: 0,
            priority: 0,
            run_at: jiff::Timestamp::now(),
            created_at: jiff::Timestamp::now(),
        })
        .exec(&mut db)
        .await
        .unwrap();

        let mut runner = Runner::new(app.clone()).register_default_jobs();
        assert!(runner.run_once().await.unwrap());

        let job =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("unknown".to_string()))
                .first()
                .exec(&mut db)
                .await
                .unwrap()
                .expect("job should still exist after failure");
        assert_eq!(job.retries, 1);
        assert!(job.run_at > jiff::Timestamp::now());
    }
}

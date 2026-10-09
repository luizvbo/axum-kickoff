//! Background worker runner.
//!
//! The runner polls `background_jobs` for due jobs and claims them with a
//! *lease*: the row's `locked_until`/`locked_by` columns record which worker
//! instance owns the job and until when. The job executes outside the
//! claiming transaction so it is free to perform its own database writes —
//! important on SQLite, where holding a write transaction open for a
//! long-running job would stall every other writer.
//!
//! If the worker crashes mid-job, the lease expires and the row becomes
//! claimable again on the next poll. Because a job may therefore run more
//! than once, [`Job`] handlers must be idempotent (the same convention
//! crates.io documents for its background jobs).
//!
//! On success the row is deleted. On failure the lease is released and the
//! job is rescheduled via `run_at` with exponential backoff; once it has been
//! attempted `max_retries` times it is marked dead (`failed_at` set) instead
//! of being rescheduled, so poison jobs cannot retry forever.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use toasty::db::{SqlPlaceholder, Transaction};
use toasty::sql;
use toasty::stmt::Value;
use toasty_core::stmt::ValueRecord;
use tokio::time::sleep;
use tracing::{error, info};
use uuid::Uuid;

use crate::app::App;
use crate::rate_limiter::timestamp_value;
use crate::worker::jobs::CleanupJob;
use crate::worker::Job;

/// A job is claimable when it is not dead and either it is unclaimed and due
/// (`locked_until IS NULL AND run_at <= now`) or its previous lease expired
/// (`locked_until < now`). The second branch reclaims rows whose worker
/// crashed mid-job — including rows stranded at `run_at = MAX` by older
/// versions of this worker that claimed jobs by overwriting `run_at`.
///
/// The predicate is designed to be served by `index_background_jobs_poll` on
/// `(queue, failed_at, locked_until, run_at)`: `failed_at IS NULL` is an
/// equality constraint on the second column, `locked_until IS NULL` /
/// `locked_until < ?` constrain the third, and `run_at <= ?` the fourth.
const PG_RESERVE_SELECT_SQL: &str = r#"
    SELECT id, job_type, data, retries
    FROM background_jobs
    WHERE queue = $1
      AND failed_at IS NULL
      AND (locked_until < $2 OR (locked_until IS NULL AND run_at <= $2))
    ORDER BY priority DESC, run_at ASC, created_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
"#;

const PG_RESERVE_UPDATE_SQL: &str =
    "UPDATE background_jobs SET locked_until = $1, locked_by = $2 WHERE id = $3";

const SQLITE_RESERVE_SQL: &str = r#"
    UPDATE background_jobs
    SET locked_until = ?1, locked_by = ?2
    WHERE id = (
        SELECT id FROM background_jobs
        WHERE queue = ?3
          AND failed_at IS NULL
          AND (locked_until < ?4 OR (locked_until IS NULL AND run_at <= ?4))
        ORDER BY priority DESC, run_at ASC, created_at ASC
        LIMIT 1
    )
    RETURNING id, job_type, data, retries
"#;

// Finalization statements are guarded by `locked_by`: if this worker's lease
// expired while the job was still running and another worker has since
// reclaimed the row, the update affects zero rows instead of clobbering the
// new claim.
const PG_DELETE_SQL: &str = "DELETE FROM background_jobs WHERE id = $1 AND locked_by = $2";
const SQLITE_DELETE_SQL: &str = "DELETE FROM background_jobs WHERE id = ?1 AND locked_by = ?2";

const PG_RETRY_SQL: &str = "UPDATE background_jobs SET retries = retries + 1, run_at = $1, locked_until = NULL, locked_by = NULL WHERE id = $2 AND locked_by = $3";
const SQLITE_RETRY_SQL: &str = "UPDATE background_jobs SET retries = retries + 1, run_at = ?1, locked_until = NULL, locked_by = NULL WHERE id = ?2 AND locked_by = ?3";

const PG_DEAD_SQL: &str = "UPDATE background_jobs SET retries = retries + 1, failed_at = $1, locked_until = NULL, locked_by = NULL WHERE id = $2 AND locked_by = $3";
const SQLITE_DEAD_SQL: &str = "UPDATE background_jobs SET retries = retries + 1, failed_at = ?1, locked_until = NULL, locked_by = NULL WHERE id = ?2 AND locked_by = ?3";

/// Default duration a claimed job stays invisible to other workers. Long
/// enough for real jobs to finish, short enough that a crashed worker's jobs
/// are reclaimed without excessive delay.
const DEFAULT_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(600);

/// Default maximum number of attempts before a failing job is marked dead.
const DEFAULT_MAX_RETRIES: i32 = 25;

/// How often the recurring [`CleanupJob`] is enqueued by the worker loop.
const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(3600);

/// `max_age_days` passed to the recurring [`CleanupJob`].
const DEFAULT_CLEANUP_MAX_AGE_DAYS: u64 = 30;

/// Handles polling, locking, and running of background jobs.
pub struct Runner {
    app: Arc<App>,
    handlers: HashMap<&'static str, Box<dyn JobHandler>>,
    poll_interval: Duration,
    queue: String,
    /// Unique identifier written to `locked_by` when this runner claims a job.
    worker_id: String,
    /// How long a claimed job stays invisible to other workers before its
    /// lease expires and the row can be reclaimed.
    visibility_timeout: Duration,
    /// Maximum number of attempts before a failing job is marked dead.
    max_retries: i32,
    /// How often the runner enqueues the recurring [`CleanupJob`].
    cleanup_interval: Duration,
    /// `max_age_days` argument for the recurring [`CleanupJob`].
    cleanup_max_age_days: u64,
    /// When the recurring cleanup job was last *successfully* enqueued
    /// (`None` = due at startup, so the first loop iteration enqueues it
    /// immediately; a failed enqueue also leaves it `None` so the next poll
    /// retries instead of waiting a full `cleanup_interval`).
    last_cleanup_enqueue: Option<Instant>,
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
            worker_id: Uuid::new_v4().to_string(),
            visibility_timeout: DEFAULT_VISIBILITY_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            cleanup_interval: DEFAULT_CLEANUP_INTERVAL,
            cleanup_max_age_days: DEFAULT_CLEANUP_MAX_AGE_DAYS,
            last_cleanup_enqueue: None,
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

    /// Identifier written to `locked_by` when this runner claims a job.
    /// Defaults to a random UUID so each worker instance is distinct.
    pub fn worker_id(mut self, worker_id: impl Into<String>) -> Self {
        self.worker_id = worker_id.into();
        self
    }

    /// How long a claimed job stays invisible to other workers before its
    /// lease expires and the row can be reclaimed. Defaults to 10 minutes.
    pub fn visibility_timeout(mut self, timeout: Duration) -> Self {
        self.visibility_timeout = timeout;
        self
    }

    /// Maximum number of attempts before a failing job is marked dead
    /// (`failed_at` set) instead of being rescheduled. Defaults to 25.
    pub fn max_retries(mut self, max_retries: i32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// How often the runner enqueues the recurring [`CleanupJob`].
    /// Defaults to one hour.
    pub fn cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }

    /// `max_age_days` argument for the recurring [`CleanupJob`].
    /// Defaults to 30.
    pub fn cleanup_max_age_days(mut self, max_age_days: u64) -> Self {
        self.cleanup_max_age_days = max_age_days;
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
        info!(queue = %self.queue, worker_id = %self.worker_id, "Background worker started");

        loop {
            // Enqueue recurring maintenance jobs when due. This runs between
            // polls so an in-flight job is never cancelled by the scheduler.
            self.enqueue_scheduled_jobs().await;

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

    /// Enqueue recurring maintenance jobs that are due — currently the
    /// periodic [`CleanupJob`]. Called on every loop iteration; it only
    /// enqueues once per `cleanup_interval`. The job lands on this runner's
    /// own queue so it is guaranteed to have a consumer.
    async fn enqueue_scheduled_jobs(&mut self) {
        let due = self
            .last_cleanup_enqueue
            .is_none_or(|instant| instant.elapsed() >= self.cleanup_interval);
        if !due {
            return;
        }

        if let Err(e) = self
            .app
            .enqueue_job_on_queue(&self.queue, CleanupJob::new(self.cleanup_max_age_days), 0)
            .await
        {
            // Leave the timestamp unset so the next poll retries the enqueue
            // rather than waiting a full `cleanup_interval`.
            self.last_cleanup_enqueue = None;
            error!(error = ?e, "Failed to enqueue scheduled cleanup job");
        } else {
            self.last_cleanup_enqueue = Some(Instant::now());
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
        let lease_until = now
            .checked_add(
                jiff::SignedDuration::try_from(self.visibility_timeout)
                    .unwrap_or(jiff::SignedDuration::MAX),
            )
            .unwrap_or(jiff::Timestamp::MAX);

        // Claim the next due job by writing this worker's lease.
        let mut reserve_tx = if placeholder == Some(SqlPlaceholder::NumberedQuestionMark) {
            db.transaction_builder()
                .mode(toasty_core::driver::operation::TransactionMode::Immediate)
                .begin()
                .await
        } else {
            db.transaction().await
        }
        .context("Failed to start worker reservation transaction")?;

        let job = match reserve_next(
            &mut reserve_tx,
            placeholder,
            &self.queue,
            now,
            lease_until,
            &self.worker_id,
        )
        .await
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

        // Finalize in a new transaction: delete on success, reschedule or
        // mark dead on failure.
        let mut finalize_tx = db
            .transaction()
            .await
            .context("Failed to start finalization transaction")?;

        match result {
            Ok(()) => {
                delete_job(&mut finalize_tx, placeholder, job.id, &self.worker_id)
                    .await
                    .context("Failed to delete completed job")?;
                finalize_tx
                    .commit()
                    .await
                    .context("Failed to commit worker finalization transaction")?;
                info!(job_id = %job.id, job_type = %job.job_type, "Job completed");
            }
            Err(e) => {
                if job.retries + 1 >= self.max_retries {
                    mark_job_dead(&mut finalize_tx, placeholder, job.id, &self.worker_id)
                        .await
                        .context("Failed to mark exhausted job as dead")?;
                    finalize_tx
                        .commit()
                        .await
                        .context("Failed to commit worker finalization transaction")?;
                    error!(
                        job_id = %job.id,
                        job_type = %job.job_type,
                        attempts = job.retries + 1,
                        error = ?e,
                        "Job exhausted retries; marked dead"
                    );
                } else {
                    // Anchor the backoff to *now* — not to the `now` captured
                    // before the handler ran. A job that executes longer than
                    // its computed delay would otherwise get a `run_at` in the
                    // past and retry in a hot loop.
                    let next_run = compute_retry_time(jiff::Timestamp::now(), job.retries);
                    update_job(
                        &mut finalize_tx,
                        placeholder,
                        job.id,
                        &self.worker_id,
                        next_run,
                    )
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
    lease_until: jiff::Timestamp,
    worker_id: &str,
) -> anyhow::Result<Option<ClaimedJob>> {
    let lease_value = timestamp_value(placeholder, lease_until);
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
                .bind(lease_value)
                .bind(worker_id)
                .bind(job.id)
                .exec(tx)
                .await
                .context("Failed to reserve next job")?;

            Ok(Some(job))
        }
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            let rows = sql::query(SQLITE_RESERVE_SQL)
                .bind(lease_value)
                .bind(worker_id)
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
    worker_id: &str,
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
        .bind(worker_id)
        .exec(tx)
        .await
        .context("Failed to delete completed job")?;
    Ok(())
}

async fn update_job(
    tx: &mut Transaction<'_>,
    placeholder: Option<SqlPlaceholder>,
    id: u64,
    worker_id: &str,
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
        .bind(worker_id)
        .exec(tx)
        .await
        .context("Failed to reschedule failed job")?;
    Ok(())
}

async fn mark_job_dead(
    tx: &mut Transaction<'_>,
    placeholder: Option<SqlPlaceholder>,
    id: u64,
    worker_id: &str,
) -> anyhow::Result<()> {
    let sql = match placeholder {
        Some(SqlPlaceholder::DollarNumber) => PG_DEAD_SQL,
        Some(SqlPlaceholder::NumberedQuestionMark) | Some(SqlPlaceholder::QuestionMark) => {
            SQLITE_DEAD_SQL
        }
        None => anyhow::bail!("raw SQL worker requires a SQL backend"),
    };

    sql::statement(sql)
        .bind(timestamp_value(placeholder, jiff::Timestamp::now()))
        .bind(id)
        .bind(worker_id)
        .exec(tx)
        .await
        .context("Failed to mark exhausted job as dead")?;
    Ok(())
}

fn compute_retry_time(now: jiff::Timestamp, retries: i32) -> jiff::Timestamp {
    let delay = 2_i64
        .checked_pow(retries.unsigned_abs().min(62))
        .unwrap_or(i64::MAX);
    // On overflow the delay is hundreds of millions of years — pinning to
    // Timestamp::MAX keeps the job parked instead of retrying immediately
    // (the old `unwrap_or(now)` fallback produced a hot retry loop).
    now.checked_add(jiff::SignedDuration::from_secs(delay))
        .unwrap_or(jiff::Timestamp::MAX)
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
            locked_until: None,
            locked_by: None,
            failed_at: None,
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
        // The lease must be released so another poll can retry the job.
        assert!(job.locked_until.is_none());
        assert!(job.locked_by.is_none());
        assert!(job.failed_at.is_none());
    }

    #[tokio::test]
    async fn test_claimed_job_records_lease() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // A job whose handler signals when it starts and blocks until
        // released lets the test observe the lease mid-claim.
        static STARTED: tokio::sync::Notify = tokio::sync::Notify::const_new();
        static RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();

        #[derive(serde::Deserialize)]
        struct SignalingJob {}

        impl crate::worker::Job for SignalingJob {
            const NAME: &'static str = "signaling";

            fn run<'a>(
                &'a self,
                _app: &'a App,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
                Box::pin(async move {
                    STARTED.notify_one();
                    RELEASE.notified().await;
                    Ok(())
                })
            }
        }

        let mut db = app.database.db_clone();
        toasty::create!(BackgroundJob {
            queue: "default".to_string(),
            job_type: "signaling".to_string(),
            data: "{}".to_string(),
            retries: 0,
            priority: 0,
            run_at: jiff::Timestamp::now(),
            created_at: jiff::Timestamp::now(),
            locked_until: None,
            locked_by: None,
            failed_at: None,
        })
        .exec(&mut db)
        .await
        .unwrap();

        let mut runner = Runner::new(app.clone())
            .register::<SignalingJob>()
            .worker_id("test-worker");

        let claimed = async {
            STARTED.notified().await;
            BackgroundJob::filter(
                BackgroundJob::fields()
                    .job_type()
                    .eq("signaling".to_string()),
            )
            .first()
            .exec(&mut db)
            .await
            .unwrap()
            .inspect(|_| RELEASE.notify_one())
        };

        let (result, mid_run_job) = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(runner.run_once(), claimed)
        })
        .await
        .expect("signaling job timed out");
        assert!(result.unwrap());
        let job = mid_run_job.expect("job should exist while it is running");
        assert_eq!(job.locked_by.as_deref(), Some("test-worker"));
        // Lease extends roughly one visibility timeout into the future.
        let locked_until = job.locked_until.expect("claimed job must have a lease");
        assert!(locked_until > jiff::Timestamp::now());
    }

    #[tokio::test]
    async fn test_stranded_job_is_reclaimed_after_lease_expiry() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // Simulate a job stranded by a crashed worker: claimed long ago, its
        // lease has expired, and `run_at` was left at the old claim sentinel
        // (Timestamp::MAX). An expired lease must make it claimable again
        // regardless of `run_at`.
        let mut db = app.database.db_clone();
        let lease_expired = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(3600))
            .unwrap();
        toasty::create!(BackgroundJob {
            queue: "default".to_string(),
            job_type: "cleanup".to_string(),
            data: serde_json::to_string(&CleanupJob::new(1)).unwrap(),
            retries: 0,
            priority: 0,
            run_at: jiff::Timestamp::MAX,
            created_at: lease_expired,
            locked_until: Some(lease_expired),
            locked_by: Some("crashed-worker".to_string()),
            failed_at: None,
        })
        .exec(&mut db)
        .await
        .unwrap();

        // A stale bucket proves the reclaimed job actually ran.
        let old = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(86400 * 2))
            .unwrap();
        toasty::create!(RateLimitBucket {
            bucket_key: "test:stranded".to_string(),
            action: "api_request".to_string(),
            bucket_id: "stranded".to_string(),
            tokens: 0,
            last_refill: old,
        })
        .exec(&mut db)
        .await
        .unwrap();

        let mut runner = Runner::new(app.clone()).register_default_jobs();
        assert!(runner.run_once().await.unwrap());

        let found = RateLimitBucket::filter(
            RateLimitBucket::fields()
                .bucket_key()
                .eq("test:stranded".to_string()),
        )
        .first()
        .exec(&mut db)
        .await
        .unwrap();
        assert!(found.is_none(), "reclaimed cleanup job should have run");

        let remaining =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("cleanup".to_string()))
                .first()
                .exec(&mut db)
                .await
                .unwrap();
        assert!(remaining.is_none());
    }

    #[tokio::test]
    async fn test_active_lease_is_not_reclaimed() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // A due job still inside another worker's lease must not be claimed.
        let mut db = app.database.db_clone();
        let lease_until = jiff::Timestamp::now()
            .checked_add(jiff::SignedDuration::from_secs(600))
            .unwrap();
        toasty::create!(BackgroundJob {
            queue: "default".to_string(),
            job_type: "cleanup".to_string(),
            data: serde_json::to_string(&CleanupJob::new(1)).unwrap(),
            retries: 0,
            priority: 0,
            run_at: jiff::Timestamp::now(),
            created_at: jiff::Timestamp::now(),
            locked_until: Some(lease_until),
            locked_by: Some("other-worker".to_string()),
            failed_at: None,
        })
        .exec(&mut db)
        .await
        .unwrap();

        let mut runner = Runner::new(app.clone()).register_default_jobs();
        assert!(!runner.run_once().await.unwrap());

        let job =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("cleanup".to_string()))
                .first()
                .exec(&mut db)
                .await
                .unwrap()
                .expect("leased job should not have been touched");
        assert_eq!(job.locked_by.as_deref(), Some("other-worker"));
    }

    #[tokio::test]
    async fn test_exhausted_job_is_marked_dead() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // "unknown" has no registered handler, so it always fails.
        let mut db = app.database.db_clone();
        toasty::create!(BackgroundJob {
            queue: "default".to_string(),
            job_type: "unknown".to_string(),
            data: "{}".to_string(),
            retries: 0,
            priority: 0,
            run_at: jiff::Timestamp::now(),
            created_at: jiff::Timestamp::now(),
            locked_until: None,
            locked_by: None,
            failed_at: None,
        })
        .exec(&mut db)
        .await
        .unwrap();

        // max_retries = 1: the first failure already exhausts the job.
        let mut runner = Runner::new(app.clone()).max_retries(1);
        assert!(runner.run_once().await.unwrap());

        let job =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("unknown".to_string()))
                .first()
                .exec(&mut db)
                .await
                .unwrap()
                .expect("dead job is kept for inspection");
        assert_eq!(job.retries, 1);
        assert!(job.failed_at.is_some());
        assert!(job.locked_until.is_none());
        assert!(job.locked_by.is_none());

        // Dead jobs are never claimed again.
        assert!(!runner.run_once().await.unwrap());
    }

    #[tokio::test]
    async fn test_worker_loop_enqueues_cleanup_job() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // The first loop iteration must enqueue the recurring cleanup job on
        // the runner's own queue. Second call must not duplicate it.
        let mut runner = Runner::new(app.clone()).queue("maintenance");
        runner.enqueue_scheduled_jobs().await;
        runner.enqueue_scheduled_jobs().await;

        let mut db = app.database.db_clone();
        let jobs =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("cleanup".to_string()))
                .exec(&mut db)
                .await
                .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].queue, "maintenance");
        assert_eq!(jobs[0].retries, 0);
        assert!(jobs[0].locked_until.is_none());
    }

    #[tokio::test]
    async fn test_failed_cleanup_enqueue_retries_on_next_poll() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // Force the enqueue to fail by removing the jobs table.
        let mut db = app.database.db_clone();
        sql::statement("DROP TABLE background_jobs")
            .exec(&mut db)
            .await
            .unwrap();

        let mut runner = Runner::new(app.clone());
        runner.enqueue_scheduled_jobs().await;

        // A failed enqueue must not be recorded: the job stays due so the
        // next poll retries instead of waiting a full `cleanup_interval`.
        assert!(runner.last_cleanup_enqueue.is_none());

        // Point the runner at a healthy database: the very next call retries
        // and the cleanup job is enqueued.
        let healthy_app = TestApp::new().await;
        runner.app = healthy_app.state.0.clone();
        runner.enqueue_scheduled_jobs().await;
        assert!(runner.last_cleanup_enqueue.is_some());

        let mut healthy_db = healthy_app.db().db_clone();
        let jobs =
            BackgroundJob::filter(BackgroundJob::fields().job_type().eq("cleanup".to_string()))
                .exec(&mut healthy_db)
                .await
                .unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn test_compute_retry_time_exponential_backoff() {
        let now = jiff::Timestamp::now();
        assert_eq!(
            compute_retry_time(now, 0),
            now + jiff::SignedDuration::from_secs(1)
        );
        assert_eq!(
            compute_retry_time(now, 4),
            now + jiff::SignedDuration::from_secs(16)
        );
    }

    #[test]
    fn test_compute_retry_time_overflow_pins_to_max() {
        let now = jiff::Timestamp::now();
        // 2^62 seconds far exceeds Timestamp range — the job is pinned to
        // MAX rather than rescheduled for *now* (the old fallback produced a
        // hot retry loop). Extreme retry counts saturate identically.
        assert_eq!(compute_retry_time(now, 62), jiff::Timestamp::MAX);
        assert_eq!(compute_retry_time(now, i32::MAX), jiff::Timestamp::MAX);
        assert_eq!(compute_retry_time(now, i32::MIN), jiff::Timestamp::MAX);
    }

    #[tokio::test]
    async fn test_run_loop_enqueues_scheduled_jobs() {
        let test_app = TestApp::new().await;
        let app = test_app.state.0.clone();

        // Drive the real worker loop: it must enqueue the recurring cleanup
        // job on its first iteration. The job has no registered handler here,
        // so after being claimed once it is merely rescheduled and the row
        // stays visible for the assertion.
        let runner = Runner::new(app.clone()).poll_interval(Duration::from_millis(10));
        let worker = tokio::spawn(runner.run());

        let mut db = app.database.db_clone();
        let deadline = Instant::now() + Duration::from_secs(10);
        let found = loop {
            let jobs =
                BackgroundJob::filter(BackgroundJob::fields().job_type().eq("cleanup".to_string()))
                    .exec(&mut db)
                    .await
                    .unwrap();
            if let Some(job) = jobs.into_iter().next() {
                break job;
            }
            assert!(
                Instant::now() < deadline,
                "worker loop did not enqueue a cleanup job"
            );
            sleep(Duration::from_millis(25)).await;
        };

        worker.abort();
        assert_eq!(found.queue, "default");
    }
}

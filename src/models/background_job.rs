//! Background job model for the worker queue.

use toasty::Model;

/// A persisted background job waiting to be processed.
///
/// Workers claim jobs with a lease (`locked_until`/`locked_by`) instead of
/// mutating `run_at`: if a worker crashes mid-job the lease expires and the
/// row becomes claimable again automatically. The poll query is served by
/// `index_background_jobs_poll` on `(queue, failed_at, locked_until, run_at)`.
#[derive(Debug, Model)]
#[index(
    name = "index_background_jobs_poll",
    queue,
    failed_at,
    locked_until,
    run_at
)]
pub struct BackgroundJob {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,
    /// Queue name the job belongs to
    pub queue: String,
    /// Job type discriminator (matches `Job::NAME`)
    pub job_type: String,
    /// JSON-encoded job payload
    pub data: String,
    /// Number of retry attempts already made
    pub retries: i32,
    /// Higher values are processed first
    pub priority: i16,
    /// Next time the job should be attempted
    pub run_at: jiff::Timestamp,
    /// Timestamp when the job was created
    pub created_at: jiff::Timestamp,
    /// Lease expiry of the current claim (NULL = not claimed). A crashed
    /// worker's job is reclaimed once this passes.
    pub locked_until: Option<jiff::Timestamp>,
    /// Identifier of the worker instance holding the lease
    pub locked_by: Option<String>,
    /// When the job exhausted its retries and was marked dead (NULL = alive)
    pub failed_at: Option<jiff::Timestamp>,
}

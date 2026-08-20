//! Background job model for the worker queue.

use toasty::Model;

/// A persisted background job waiting to be processed.
#[derive(Debug, Model)]
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
}

//! Background worker queue.
//!
//! Provides the [`Job`] trait, a [`Runner`] that polls `background_jobs`, and
//! built-in example jobs such as [`CleanupJob`].

pub mod jobs;
pub mod runner;

pub use jobs::{CleanupJob, Job};
pub use runner::Runner;

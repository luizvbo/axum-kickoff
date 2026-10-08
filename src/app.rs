//! Application-wide components in a struct accessible from each request

use crate::config;
use crate::db::Database;
{% if metrics %}#[cfg(feature = "metrics")]
use crate::metrics::InstanceMetrics;
{% endif %}use crate::models::BackgroundJob;
use crate::rate_limiter::RateLimiter;
use crate::storage::Storage;
use crate::worker::Job;
use secrecy::ExposeSecret;
use std::sync::Arc;
use std::time::Duration;

use derive_more::Deref;

/// The `App` struct holds the main components of the application like
/// the database connection pool and configurations
pub struct App {
    /// The server configuration
    pub config: Arc<config::Server>,
    /// The database connection pool
    pub database: Database,
    /// Storage backend for file uploads and static assets
    pub storage: Storage,
{% if metrics %}    /// Instance metrics for monitoring (available with `metrics` feature)
    #[cfg(feature = "metrics")]
    pub metrics: std::sync::Arc<InstanceMetrics>,
{% endif %}    /// Session key for signing cookies
    pub session_key: cookie::Key,
    /// Rate limiter for API request throttling
    pub rate_limiter: RateLimiter,
    /// Shared HTTP client for outbound requests (OAuth providers)
    pub http_client: reqwest::Client,
}

impl App {
    /// Create a new App instance with the given configuration and database
    pub fn new(config: config::Server, database: Database) -> anyhow::Result<Self> {
        let session_key = cookie::Key::derive_from(config.session_key.expose_secret().as_bytes());
        let storage = Storage::from_config(&config.storage_config)?;

        // Initialize rate limiter from server config or default configuration
        let rate_limiter = RateLimiter::new(config.rate_limiter_config.clone(), database.clone());

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            config: Arc::new(config),
            database,
            storage,
{% if metrics %}            #[cfg(feature = "metrics")]
            metrics: std::sync::Arc::new(InstanceMetrics::new()),
{% endif %}            session_key,
            rate_limiter,
            http_client,
        })
    }

    /// Get the server's IP address
    pub fn ip(&self) -> std::net::IpAddr {
        self.config.ip
    }

    /// Get the server's port
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Get the domain name
    pub fn domain_name(&self) -> &str {
        &self.config.domain_name
    }

    /// Get the database
    pub fn db(&self) -> &Database {
        &self.database
    }

    /// Get a database handle for read operations.
    pub fn db_read(&self) -> Database {
        self.database.clone()
    }

    /// Get a database handle for write operations.
    pub fn db_write(&self) -> Database {
        self.database.clone()
    }

    /// Enqueue a background job in the default queue.
    pub async fn enqueue_job<J: Job + serde::Serialize>(&self, job: J) -> anyhow::Result<()> {
        self.enqueue_job_on_queue("default", job, 0).await
    }

    /// Enqueue a background job with an explicit priority.
    pub async fn enqueue_job_with_priority<J: Job + serde::Serialize>(
        &self,
        job: J,
        priority: i16,
    ) -> anyhow::Result<()> {
        self.enqueue_job_on_queue("default", job, priority).await
    }

    /// Enqueue a background job on a specific queue with an explicit priority.
    pub async fn enqueue_job_on_queue<J: Job + serde::Serialize>(
        &self,
        queue: &str,
        job: J,
        priority: i16,
    ) -> anyhow::Result<()> {
        let data = serde_json::to_string(&job)?;
        let mut db = self.database.db_clone();

        toasty::create!(BackgroundJob {
            queue: queue.to_string(),
            job_type: J::NAME.to_string(),
            data,
            retries: 0,
            priority,
            run_at: jiff::Timestamp::now(),
            created_at: jiff::Timestamp::now(),
            locked_until: None,
            locked_by: None,
            failed_at: None,
        })
        .exec(&mut db)
        .await?;

        Ok(())
    }

    /// Get the storage backend
    pub fn storage(&self) -> &Storage {
        &self.storage
    }
}

#[derive(Clone, Deref)]
pub struct AppState(pub Arc<App>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_clone() {
        // This test verifies AppState can be cloned
        // We can't create a real App without full setup, but we can test the type
        // This is a compile-time test to ensure the Clone derive works
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }

    #[test]
    fn test_app_state_deref() {
        // This test verifies AppState derefs to App
        // We can't create a real App without full setup, but we can test the type
        // This is a compile-time test to ensure the Deref derive works
        fn assert_deref<T: std::ops::Deref>() {}
        assert_deref::<AppState>();
    }

    #[test]
    fn test_app_state_send_sync() {
        // Verify AppState is Send and Sync (required for Arc)
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<AppState>();
        assert_sync::<AppState>();
    }
}

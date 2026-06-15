//! Application-wide components in a struct accessible from each request

use crate::config;
use crate::db::Database;
#[cfg(feature = "metrics")]
use crate::metrics::InstanceMetrics;
use crate::rate_limiter::{LimitedAction, RateLimiter, RateLimiterConfig};
use crate::storage::Storage;
use std::collections::HashMap;
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
    /// Instance metrics for monitoring (available with `metrics` feature)
    #[cfg(feature = "metrics")]
    pub metrics: InstanceMetrics,
    /// Session key for signing cookies
    pub session_key: cookie::Key,
    /// Rate limiter for API request throttling
    pub rate_limiter: RateLimiter,
}

impl App {
    /// Create a new App instance with the given configuration and database
    pub fn new(config: config::Server, database: Database) -> Self {
        let session_key = config.session_key.clone();
        let storage = Storage::from_config(&config.storage_config);

        // Initialize rate limiter with default configuration
        let mut rate_limit_config = HashMap::new();
        for action in LimitedAction::VARIANTS {
            rate_limit_config.insert(
                action,
                RateLimiterConfig {
                    rate: Duration::from_secs(action.default_rate_seconds()),
                    burst: action.default_burst(),
                },
            );
        }
        let rate_limiter = RateLimiter::new(rate_limit_config);

        Self {
            config: Arc::new(config),
            database,
            storage,
            #[cfg(feature = "metrics")]
            metrics: InstanceMetrics::new(),
            session_key,
            rate_limiter,
        }
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

    /// Get the storage backend
    pub fn storage(&self) -> &Storage {
        &self.storage
    }
}

#[derive(Clone, Deref)]
pub struct AppState(pub Arc<App>);

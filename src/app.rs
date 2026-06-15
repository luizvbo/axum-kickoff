//! Application-wide components in a struct accessible from each request

use crate::config;
use crate::db::Database;
use crate::storage::Storage;
#[cfg(feature = "metrics")]
use crate::metrics::InstanceMetrics;
use std::sync::Arc;

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
}

impl App {
    /// Create a new App instance with the given configuration and database
    pub fn new(config: config::Server, database: Database) -> Self {
        let session_key = config.session_key.clone();
        let storage = Storage::from_config(&config.storage_config);
        Self {
            config: Arc::new(config),
            database,
            storage,
            #[cfg(feature = "metrics")]
            metrics: InstanceMetrics::new(),
            session_key,
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

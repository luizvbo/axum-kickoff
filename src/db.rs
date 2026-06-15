//! Database layer using Toasty ORM
//!
//! This module provides database connection management and schema setup.

use crate::config::DatabaseConfig;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use toasty::Db;

/// Database wrapper with connection management
#[derive(Clone)]
pub struct Database {
    db: Arc<Db>,
}

impl Database {
    /// Create a new database connection from configuration
    ///
    /// Uses connection pooling with sensible defaults:
    /// - max_pool_size: num_cpus * 2
    /// - pool_wait_timeout: 5 seconds
    /// - pool_create_timeout: 10 seconds
    /// - pool_health_check_interval: 60 seconds
    /// - pool_pre_ping: true (check connection health before use)
    pub async fn from_config(config: &DatabaseConfig) -> Result<Self> {
        let db = Db::builder()
            .models(toasty::models!(crate::*))
            .max_pool_size(num_cpus::get() * 2)
            .pool_wait_timeout(Some(Duration::from_secs(5)))
            .pool_create_timeout(Some(Duration::from_secs(10)))
            .pool_health_check_interval(Some(Duration::from_secs(60)))
            .pool_pre_ping(true)
            .connect(&config.url)
            .await?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Create a new database connection from configuration with custom pool settings
    pub async fn from_config_with_pool(
        config: &DatabaseConfig,
        max_pool_size: usize,
        wait_timeout: Option<Duration>,
        create_timeout: Option<Duration>,
    ) -> Result<Self> {
        let db = Db::builder()
            .models(toasty::models!(crate::*))
            .max_pool_size(max_pool_size)
            .pool_wait_timeout(wait_timeout)
            .pool_create_timeout(create_timeout)
            .pool_health_check_interval(Some(Duration::from_secs(60)))
            .pool_pre_ping(true)
            .connect(&config.url)
            .await?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Get a reference to the underlying Toasty Db
    pub fn db(&self) -> &Db {
        &self.db
    }

    /// Get an Arc reference to the underlying Toasty Db
    pub fn db_arc(&self) -> Arc<Db> {
        self.db.clone()
    }
}

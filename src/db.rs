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

    /// Get a cloned Db handle for mutations
    pub fn db_clone(&self) -> Db {
        self.db.as_ref().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_database_from_config() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig { url: db_url.clone() };
        let db = Database::from_config(&config)
            .await
            .expect("Failed to create database");

        // Test that we can get references to the underlying Db
        let _db_ref = db.db();
        let _db_arc = db.db_arc();
        let _db_clone = db.db_clone();
    }

    #[tokio::test]
    async fn test_database_from_config_with_custom_pool() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig { url: db_url.clone() };
        let db = Database::from_config_with_pool(
            &config,
            4, // max_pool_size
            Some(Duration::from_secs(3)), // wait_timeout
            Some(Duration::from_secs(8)), // create_timeout
        )
        .await
        .expect("Failed to create database with custom pool");

        // Verify the database was created successfully
        let _db_ref = db.db();
    }

    #[tokio::test]
    async fn test_database_db_arc_returns_same_instance() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig { url: db_url.clone() };
        let db = Database::from_config(&config)
            .await
            .expect("Failed to create database");

        let arc1 = db.db_arc();
        let arc2 = db.db_arc();

        // Both Arcs should point to the same underlying Db
        assert!(Arc::ptr_eq(&db.db, &arc1));
        assert!(Arc::ptr_eq(&db.db, &arc2));
    }

    #[tokio::test]
    async fn test_database_db_clone_creates_new_handle() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig { url: db_url.clone() };
        let db = Database::from_config(&config)
            .await
            .expect("Failed to create database");

        let clone1 = db.db_clone();
        let clone2 = db.db_clone();

        // Clones should be independent handles
        // (they point to the same pool but are different Db instances)
        let _ = clone1;
        let _ = clone2;
    }

    #[tokio::test]
    async fn test_database_none_timeouts() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig { url: db_url.clone() };
        let db = Database::from_config_with_pool(
            &config,
            2,
            None, // wait_timeout
            None, // create_timeout
        )
        .await
        .expect("Failed to create database with None timeouts");

        let _db_ref = db.db();
    }
}

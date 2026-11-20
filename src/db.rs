//! Database layer using Toasty ORM
//!
//! This module provides database connection management and schema setup.

use crate::config::DatabaseConfig;
use anyhow::{Context, Result};
use secrecy::ExposeSecret;
use std::sync::Arc;
use std::time::Duration;
use toasty::migration::MigrationSet;
use toasty::Db;

/// Migrations embedded at compile time so a deployed binary can apply them
/// without the `migrations/` directory on disk.
const MIGRATIONS: MigrationSet = toasty::embed_migrations!("{% if database == "postgresql" %}migrations-postgres{% else %}migrations{% endif %}");

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
        Self::from_config_with_pool(
            config,
            num_cpus::get() * 2,
            Some(Duration::from_secs(5)),
            Some(Duration::from_secs(10)),
        )
        .await
    }

    /// Create a new database connection from configuration with custom pool settings
    pub async fn from_config_with_pool(
        config: &DatabaseConfig,
        max_pool_size: usize,
        wait_timeout: Option<Duration>,
        create_timeout: Option<Duration>,
    ) -> Result<Self> {
        let mut db = Db::builder()
            .models(toasty::models!(crate::*))
            .max_pool_size(max_pool_size)
            .pool_wait_timeout(wait_timeout)
            .pool_create_timeout(create_timeout)
            .pool_health_check_interval(Some(Duration::from_secs(60)))
            .pool_pre_ping(true)
            .connect(config.connect_url()?.expose_secret())
            .await
            .with_context(|| format!("failed to connect to database at {}", config.describe()))?;

        // `journal_mode` is persisted in the database file header, so one
        // PRAGMA on any connection switches the file to WAL for every current
        // and future connection. Ignored for in-memory databases, which
        // report `memory` and can't use WAL.
        // shortcut: per-connection pragmas can't be set through toasty's
        // pool — upgrade when the driver gains a connection-init hook.
        // Consequences: `foreign_keys` stays OFF (`FOREIGN KEY` / `REFERENCES`
        // clauses in hand-written SQLite migrations are not enforced;
        // PostgreSQL enforces them normally). The `busy_timeout` query param
        // appended by `DatabaseConfig::connect_url` *is* honored by the
        // driver — see `test_sqlite_busy_timeout_param_is_applied`.
        let _ = toasty::sql::statement("PRAGMA journal_mode=WAL")
            .exec(&mut db)
            .await;

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

    /// Apply pending migrations embedded in the binary.
    ///
    /// Unlike the `toasty` CLI this needs no `Toasty.toml`, `history.toml`,
    /// or `.sql` files on disk — it works in a release image that ships only
    /// the binary.
    pub async fn migrate(&self) -> Result<()> {
        MIGRATIONS
            .apply(&self.db)
            .await
            .context("failed to apply embedded migrations")?;
        Ok(())
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

        let config = DatabaseConfig {
            url: db_url.clone().into(),
        };
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

        let config = DatabaseConfig {
            url: db_url.clone().into(),
        };
        let db = Database::from_config_with_pool(
            &config,
            4,                            // max_pool_size
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

        let config = DatabaseConfig {
            url: db_url.clone().into(),
        };
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

        let config = DatabaseConfig {
            url: db_url.clone().into(),
        };
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

    // `connect_url` appends `busy_timeout=5000` to SQLite URLs; this pins
    // that the driver actually applies it (verified empirically — reading
    // the driver source suggested it might drop query params).
    #[tokio::test]
    async fn test_sqlite_busy_timeout_param_is_applied() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig { url: db_url.into() };
        let db = Database::from_config(&config)
            .await
            .expect("Failed to create database");

        let mut conn = db.db_clone();
        let rows = toasty::sql::query("PRAGMA busy_timeout")
            .exec(&mut conn)
            .await
            .expect("PRAGMA busy_timeout failed");

        let busy_timeout = match rows.into_iter().next() {
            Some(toasty::stmt::Value::Record(record)) => {
                record.fields.into_iter().next().and_then(|v| v.to_u64())
            }
            other => panic!("Expected record row, got {other:?}"),
        };
        assert_eq!(busy_timeout, Some(5000));
    }

    #[tokio::test]
    async fn test_database_none_timeouts() {
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        let config = DatabaseConfig {
            url: db_url.clone().into(),
        };
        let db = Database::from_config_with_pool(
            &config, 2, None, // wait_timeout
            None, // create_timeout
        )
        .await
        .expect("Failed to create database with None timeouts");

        let _db_ref = db.db();
    }
}

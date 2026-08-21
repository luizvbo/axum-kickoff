//! Optional PostgreSQL test database harness
//!
//! By default tests use a temporary SQLite database file. Set
//! `TEST_DATABASE_URL` to point at a PostgreSQL database (or any other URL
//! supported by Toasty) to run the suite against a different backend.
//!
//! When running against PostgreSQL in parallel, the caller is responsible for
//! providing a fresh database or schema per test process (for example via
//! `psql` in a wrapper script or a test service like `testcontainers`).

use crate::config::DatabaseConfig;
use crate::db::Database;
use secrecy::SecretString;
use tempfile::NamedTempFile;

/// Test database handle that keeps any temporary file alive for the
/// duration of the test.
pub struct TestDatabase {
    pub config: DatabaseConfig,
    _temp_file: Option<NamedTempFile>,
}

impl Default for TestDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDatabase {
    /// Create a new test database configuration.
    ///
    /// If `TEST_DATABASE_URL` is set, it is used as-is. Otherwise a temporary
    /// SQLite file is created.
    pub fn new() -> Self {
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            return Self {
                config: DatabaseConfig {
                    url: SecretString::from(url),
                },
                _temp_file: None,
            };
        }

        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        Self {
            config: DatabaseConfig {
                url: SecretString::from(db_url),
            },
            _temp_file: Some(db_file),
        }
    }

    /// Connect to the database and apply migrations.
    pub async fn connect_and_migrate(&self) -> Database {
        let db = Database::from_config(&self.config)
            .await
            .expect("Failed to connect to test database");

        db.migrate()
            .await
            .expect("Failed to apply test database migrations");

        db
    }
}

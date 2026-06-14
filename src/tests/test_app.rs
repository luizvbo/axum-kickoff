//! Test application builder
//!
//! Adapted from crates.io's TestApp to provide a test-ready application
//! with in-memory SQLite database and simplified configuration.

use crate::app::{App, AppState};
use crate::config::Server;
use crate::db::Database;
use crate::router;
use axum::Router;
use std::sync::Arc;
use tempfile::NamedTempFile;

/// Test application with isolated database
pub struct TestApp {
    /// The axum Router for making requests
    pub router: Router<()>,
    /// The application state
    pub state: AppState,
    /// The database connection
    pub db: Database,
    /// The temp file holding the SQLite database (kept alive for test duration)
    _db_file: NamedTempFile,
    /// The application configuration
    pub config: Server,
}

impl TestApp {
    /// Create a new test application with an in-memory SQLite database
    pub fn new() -> Self {
        // Create a temporary file for the SQLite database
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        // Set up test configuration
        let config = Self::test_config();

        // Create database connection
        let db_config = crate::config::DatabaseConfig {
            url: db_url.clone(),
        };
        
        let db = tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime")
            .block_on(Database::from_config(&db_config))
            .expect("Failed to connect to test database");

        // Create app state
        let app = App::new(config.clone(), db.clone());

        // Build router with test configuration
        let state = AppState(Arc::new(app));
        let router = router::build_axum_router(state.clone());

        Self {
            router,
            state,
            db,
            _db_file: db_file,
            config,
        }
    }

    /// Create test configuration with minimal required settings
    fn test_config() -> Server {
        use crate::config::base::Base;
        use crate::config::AllowedOrigins;
        use crate::Env;
        use std::net::IpAddr;

        // Generate a random session key for tests
        let session_key = cookie::Key::generate();

        Server {
            base: Base {
                env: Env::Test,
            },
            ip: IpAddr::from([127, 0, 0, 1]),
            port: 8888,
            max_blocking_threads: None,
            domain_name: "localhost".to_string(),
            allowed_origins: AllowedOrigins::from_str("http://localhost:3000"),
            blocked_ips: Default::default(),
            blocked_routes: Default::default(),
            blocked_traffic: Default::default(),
            session_key,
        }
    }

    /// Get a reference to the database
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Get the app state (useful for extractors that need it)
    pub fn state(&self) -> AppState {
        // Extract the state from the router
        // This is a simplification - in practice you might need to store it separately
        // For now, we'll create a new state from the existing components
        let app = App::new(self.config.clone(), self.db.clone());
        AppState(Arc::new(app))
    }
}

impl Default for TestApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = TestApp::new();
        assert_eq!(app.config.port, 8888);
        assert_eq!(app.config.domain_name, "localhost");
    }

    #[test]
    fn test_app_default() {
        let app = TestApp::default();
        assert_eq!(app.config.port, 8888);
    }
}

//! Test application builder
//!
//! Adapted from crates.io's TestApp to provide a test-ready application
//! with in-memory SQLite database and simplified configuration.

use crate::app::{App, AppState};
use crate::config::AllowedOrigins;
use crate::config::Server;
use crate::db::Database;
use crate::storage::StorageConfig;
use crate::tests::builders::{ApiTokenBuilder, UserBuilder};
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
    pub async fn new() -> Self {
        // Create a temporary file for the SQLite database
        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        // Set up test configuration
        let config = Self::test_config();

        // Create database connection
        let db_config = crate::config::DatabaseConfig {
            url: db_url.clone(),
        };

        let db = Database::from_config(&db_config)
            .await
            .expect("Failed to connect to test database");

        // Create app state
        let app = App::new(config.clone(), db.clone());

        // Build router with test configuration and middleware
        let app_arc = Arc::new(app);
        let state = AppState(app_arc.clone());
        let router = crate::build_handler(app_arc);

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
        use crate::Env;
        use std::net::IpAddr;

        // Generate a random session key for tests
        let session_key = cookie::Key::generate();

        Server {
            base: Base { env: Env::Test },
            ip: IpAddr::from([127, 0, 0, 1]),
            port: 8888,
            max_blocking_threads: None,
            domain_name: "localhost".to_string(),
            allowed_origins: AllowedOrigins::parse("http://localhost:3000"),
            blocked_ips: Default::default(),
            blocked_routes: Default::default(),
            blocked_traffic: Default::default(),
            session_key,
            trusted_proxies: vec!["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()],
            gh_client_id: "test_client_id".to_string(),
            gh_client_secret: "test_client_secret".to_string(),
            gh_redirect_uri: "http://localhost:8888/api/v1/auth/github/callback".to_string(),
            storage_config: StorageConfig::local_filesystem("./test_uploads"),
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

    /// Create a new user builder
    pub fn user_builder(&self, gh_login: &str) -> UserBuilder {
        UserBuilder::new(gh_login)
    }

    /// Create a new API token builder
    pub fn token_builder(&self, user_id: u64, name: &str) -> ApiTokenBuilder {
        ApiTokenBuilder::new(user_id, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        let app = TestApp::new().await;
        assert_eq!(app.config.port, 8888);
        assert_eq!(app.config.domain_name, "localhost");
    }
}

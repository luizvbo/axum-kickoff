//! Test application builder
//!
//! Adapted from crates.io's TestApp to provide a test-ready application
//! with in-memory SQLite database and simplified configuration.

use crate::app::{App, AppState};
use crate::config::AllowedOrigins;
use crate::config::LogFormat;
use crate::config::Server;
use crate::db::Database;
use crate::storage::StorageConfig;
use crate::tests::builders::{ApiTokenBuilder, PostBuilder, UserBuilder};
use crate::tests::test_db::TestDatabase;
use axum::Router;
use secrecy::SecretString;
use std::sync::Arc;

/// Test application with isolated database
pub struct TestApp {
    /// The axum Router for making requests
    pub router: Router<()>,
    /// The application state
    pub state: AppState,
    /// The database connection
    pub db: Database,
    /// Test database handle (keeps any temp file alive for the test duration)
    pub test_db: TestDatabase,
    /// The application configuration
    pub config: Server,
}

impl TestApp {
    /// Create a new test application with an in-memory SQLite database
    pub async fn new() -> Self {
        let config = Self::test_config();
        Self::with_config(config).await
    }

    /// Create a new test application with a custom configuration
    pub async fn with_config(config: Server) -> Self {
        // Create the test database (SQLite by default, PostgreSQL when
        // `TEST_DATABASE_URL` is set and the `postgresql` feature is enabled).
        let test_db = TestDatabase::new();
        let db = test_db.connect_and_migrate().await;

        // Create app state
        let app = App::new(config.clone(), db.clone()).expect("Failed to create test app");

        // Build router with test configuration and middleware
        let app_arc = Arc::new(app);
        let state = AppState(app_arc.clone());
        let router = crate::build_handler(app_arc);

        Self {
            router,
            state,
            db,
            test_db,
            config,
        }
    }

    /// Create test configuration with minimal required settings
    pub fn test_config() -> Server {
        use crate::config::base::Base;
        use crate::rate_limiter::{LimitedAction, RateLimiterConfig};
        use crate::Env;
        use std::collections::HashMap;
        use std::net::IpAddr;
        use std::time::Duration;

        // Use a deterministic secret string for cookie key derivation in tests.
        // It should be at least 64 bytes long to satisfy the cookie key requirements.
        let session_key = SecretString::from(
            "test-session-key-that-is-long-enough-for-cookie-key-derivation-foobar".to_string(),
        );

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
            trusted_proxies: vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()],
            gh_client_id: "test_client_id".to_string(),
            gh_client_secret: SecretString::from("test_client_secret"),
            gh_redirect_uri: "http://localhost:8888/api/v1/auth/github/callback".to_string(),
            storage_config: StorageConfig::local_filesystem("./test_uploads"),
            rate_limiter_config: LimitedAction::VARIANTS
                .iter()
                .map(|&a| {
                    (
                        a,
                        RateLimiterConfig {
                            rate: Duration::from_millis(1),
                            burst: 10_000,
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            metrics_token: None,
            sentry_dsn: None,
            log_format: LogFormat::Pretty,
        }
    }

    /// Get a reference to the database
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Get the app state (useful for extractors that need it)
    pub fn state(&self) -> AppState {
        self.state.clone()
    }

    /// Create a new user builder
    pub fn user_builder(&self, gh_login: &str) -> UserBuilder {
        UserBuilder::new(gh_login)
    }

    /// Create a new API token builder
    pub fn token_builder(&self, user_id: u64, name: &str) -> ApiTokenBuilder {
        ApiTokenBuilder::new(user_id, name)
    }

    /// Create a new post builder
    pub fn post_builder(&self, user_id: u64, title: &str) -> PostBuilder {
        PostBuilder::new(user_id, title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{AnonymousUser, RequestHelper};
    use regex::Regex;

    fn sanitize_home_page(html: &str) -> String {
        let csrf = Regex::new(r#"content="[A-Za-z0-9]{32}""#).unwrap();
        let csrf_json = Regex::new(r#""X-CSRF-Token": "[A-Za-z0-9]{32}""#).unwrap();
        let nonce = Regex::new(r#"nonce="[A-Za-z0-9+/=]{20,26}""#).unwrap();

        let html = csrf.replace_all(html, r#"content="[CSRF_TOKEN]""#);
        let html = csrf_json.replace_all(&html, r#""X-CSRF-Token": "[CSRF_TOKEN]""#);
        nonce
            .replace_all(&html, r#"nonce="[CSP_NONCE]""#)
            .into_owned()
    }

    #[tokio::test]
    async fn test_app_creation() {
        let app = TestApp::new().await;
        assert_eq!(app.config.port, 8888);
        assert_eq!(app.config.domain_name, "localhost");
    }

    #[tokio::test]
    async fn home_page_snapshot() {
        let app = TestApp::new().await;
        let anon = AnonymousUser::new(app);

        let response = anon.get::<()>("/").await;
        response.assert_success();

        let body = sanitize_home_page(&response.into_string().await);
        insta::assert_snapshot!(body);
    }
}

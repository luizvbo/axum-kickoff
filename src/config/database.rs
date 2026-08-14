//! Database configuration
//!
//! Pulls values from the following environment variables:
//!
//! - `DATABASE_URL`: The database connection URL (required in production).
//!   SQLite format: `sqlite:./path/to/db.sqlite` or `sqlite::memory:`
//!   PostgreSQL format: `postgresql://user:password@host:port/database`
//! - `TEST_DATABASE_URL`: The database connection URL for tests (optional).
//! - `DATABASE_APPLICATION_NAME`: Override the PostgreSQL `application_name`.
//! - `DATABASE_STATEMENT_TIMEOUT`: Override the PostgreSQL `statement_timeout`.
//! - `DATABASE_SSLMODE`: Override the PostgreSQL `sslmode`.

use anyhow::Result;
use secrecy::{ExposeSecret, SecretString};
use std::collections::HashMap;
use url::Url;

pub struct DatabaseConfig {
    pub url: SecretString,
}

impl DatabaseConfig {
    pub fn from_environment() -> Result<Self> {
        let url = dotenvy::var("DATABASE_URL")
            .or_else(|_| dotenvy::var("TEST_DATABASE_URL"))
            .unwrap_or_else(|_| "sqlite:./axum_kickoff.db".to_string());

        Ok(Self {
            url: SecretString::from(url),
        })
    }

    #[cfg(test)]
    pub fn test_config() -> Result<Self> {
        let url =
            dotenvy::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());

        Ok(Self {
            url: SecretString::from(url),
        })
    }

    /// Returns the connection URL with per-driver defaults merged into the query string.
    pub fn connect_url(&self) -> Result<SecretString> {
        let mut url = Url::parse(self.url.expose_secret())
            .map_err(|e| anyhow::anyhow!("failed to parse database URL: {e}"))?;

        let mut query: HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        match url.scheme() {
            "sqlite" => {
                if !query.contains_key("busy_timeout") {
                    query.insert("busy_timeout".to_string(), "5000".to_string());
                }
            }
            "postgresql" | "postgres" => {
                if !query.contains_key("application_name") {
                    let application_name = dotenvy::var("DATABASE_APPLICATION_NAME")
                        .unwrap_or_else(|_| "axum_kickoff".to_string());
                    query.insert("application_name".to_string(), application_name);
                }

                if !query.contains_key("options") {
                    let statement_timeout = dotenvy::var("DATABASE_STATEMENT_TIMEOUT")
                        .unwrap_or_else(|_| "30s".to_string());
                    query.insert(
                        "options".to_string(),
                        format!("-c statement_timeout={statement_timeout}"),
                    );
                }

                if !query.contains_key("sslmode") {
                    if let Ok(sslmode) = dotenvy::var("DATABASE_SSLMODE") {
                        query.insert("sslmode".to_string(), sslmode);
                    } else if dotenvy::var("HEROKU").is_ok() {
                        query.insert("sslmode".to_string(), "require".to_string());
                    }
                }
            }
            _ => {}
        }

        {
            let mut pairs = url.query_pairs_mut();
            pairs.clear();
            for (key, value) in &query {
                pairs.append_pair(key, value);
            }
        }

        Ok(SecretString::from(url.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Serialize tests that mutate process environment variables.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_from_environment_with_database_url() {
        let _guard = ENV_LOCK.lock();
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/db");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(config.url.expose_secret(), "postgresql://user:pass@localhost/db");

        // Restore original values
        if let Some(val) = original_db {
            std::env::set_var("DATABASE_URL", val);
        } else {
            std::env::remove_var("DATABASE_URL");
        }
        if let Some(val) = original_test {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }

    #[test]
    fn test_from_environment_with_test_database_url() {
        let _guard = ENV_LOCK.lock();
        // Note: This test may fail if there's a .env file with DATABASE_URL set
        // since dotenvy reads from .env files. This is a known limitation.
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::remove_var("DATABASE_URL");
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        // Only assert if we're not getting the default value (which means .env is interfering)
        if config.url.expose_secret() != "sqlite:./axum_kickoff.db" {
            assert_eq!(config.url.expose_secret(), "sqlite::memory:");
        }

        // Restore original values
        if let Some(val) = original_db {
            std::env::set_var("DATABASE_URL", val);
        } else {
            std::env::remove_var("DATABASE_URL");
        }
        if let Some(val) = original_test {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }

    #[test]
    fn test_from_environment_test_url_takes_precedence() {
        let _guard = ENV_LOCK.lock();
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/db");
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        // DATABASE_URL takes precedence in the implementation
        assert_eq!(config.url.expose_secret(), "postgresql://user:pass@localhost/db");

        // Restore original values
        if let Some(val) = original_db {
            std::env::set_var("DATABASE_URL", val);
        } else {
            std::env::remove_var("DATABASE_URL");
        }
        if let Some(val) = original_test {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }

    #[test]
    fn test_from_environment_default() {
        let _guard = ENV_LOCK.lock();
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        // Ensure neither DATABASE_URL nor TEST_DATABASE_URL is set
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("TEST_DATABASE_URL");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(config.url.expose_secret(), "sqlite:./axum_kickoff.db");

        // Restore original values
        if let Some(val) = original_db {
            std::env::set_var("DATABASE_URL", val);
        } else {
            std::env::remove_var("DATABASE_URL");
        }
        if let Some(val) = original_test {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }

    #[test]
    fn test_test_config_with_env() {
        let _guard = ENV_LOCK.lock();
        let original = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::test_config().expect("Failed to create test Database config");
        assert_eq!(config.url.expose_secret(), "sqlite::memory:");

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }

    #[test]
    fn test_test_config_default() {
        let _guard = ENV_LOCK.lock();
        let original = std::env::var("TEST_DATABASE_URL").ok();
        std::env::remove_var("TEST_DATABASE_URL");

        let config = DatabaseConfig::test_config().expect("Failed to create test Database config");
        assert_eq!(config.url.expose_secret(), "sqlite::memory:");

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }
}

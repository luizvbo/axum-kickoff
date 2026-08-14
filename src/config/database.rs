//! Database configuration
//!
//! Pulls values from the following environment variables:
//!
//! - `DATABASE_URL`: The database connection URL (required in production).
//!   SQLite format: `sqlite:./path/to/db.sqlite` or `sqlite::memory:`
//!   PostgreSQL format: `postgresql://user:password@host:port/database`
//! - `TEST_DATABASE_URL`: The database connection URL for tests (optional).

use crate::config::env;
use anyhow::Result;
use secrecy::SecretString;

pub struct DatabaseConfig {
    pub url: SecretString,
}

impl DatabaseConfig {
    pub fn from_environment() -> Result<Self> {
        let url = if let Some(url) = env::var("DATABASE_URL")? {
            url
        } else if let Some(url) = env::var("TEST_DATABASE_URL")? {
            url
        } else {
            "sqlite:./axum_kickoff.db".to_string()
        };

        Ok(Self {
            url: SecretString::from(url),
        })
    }

    #[cfg(test)]
    pub fn test_config() -> Result<Self> {
        let url = env::var("TEST_DATABASE_URL")?.unwrap_or_else(|| "sqlite::memory:".to_string());

        Ok(Self {
            url: SecretString::from(url),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    // Serialize tests that mutate process environment variables.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn save_and_clear_database_env() -> (Option<String>, Option<String>) {
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("TEST_DATABASE_URL");
        (original_db, original_test)
    }

    fn restore_database_env(original_db: Option<String>, original_test: Option<String>) {
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
    fn test_from_environment_with_database_url() {
        let _guard = ENV_LOCK.lock();
        let (original_db, original_test) = save_and_clear_database_env();
        std::env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/db");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(
            config.url.expose_secret(),
            "postgresql://user:pass@localhost/db"
        );

        restore_database_env(original_db, original_test);
    }

    #[test]
    fn test_from_environment_with_test_database_url() {
        let _guard = ENV_LOCK.lock();
        let (original_db, original_test) = save_and_clear_database_env();
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(config.url.expose_secret(), "sqlite::memory:");

        restore_database_env(original_db, original_test);
    }

    #[test]
    fn test_from_environment_test_url_takes_precedence() {
        let _guard = ENV_LOCK.lock();
        let (original_db, original_test) = save_and_clear_database_env();
        std::env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/db");
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(
            config.url.expose_secret(),
            "postgresql://user:pass@localhost/db"
        );

        restore_database_env(original_db, original_test);
    }

    #[test]
    fn test_from_environment_default() {
        let _guard = ENV_LOCK.lock();
        let (original_db, original_test) = save_and_clear_database_env();

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(config.url.expose_secret(), "sqlite:./axum_kickoff.db");

        restore_database_env(original_db, original_test);
    }

    #[test]
    fn test_test_config_with_env() {
        let _guard = ENV_LOCK.lock();
        let original = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::test_config().expect("Failed to create test Database config");
        assert_eq!(config.url.expose_secret(), "sqlite::memory:");

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

        if let Some(val) = original {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }
}

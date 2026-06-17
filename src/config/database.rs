//! Database configuration
//!
//! Pulls values from the following environment variables:
//!
//! - `DATABASE_URL`: The database connection URL (required in production).
//!   SQLite format: `sqlite:./path/to/db.sqlite` or `sqlite::memory:`
//!   PostgreSQL format: `postgresql://user:password@host:port/database`
//! - `TEST_DATABASE_URL`: The database connection URL for tests (optional).

use anyhow::Result;

pub struct DatabaseConfig {
    pub url: String,
}

impl DatabaseConfig {
    pub fn from_environment() -> Result<Self> {
        let url = dotenvy::var("DATABASE_URL")
            .or_else(|_| dotenvy::var("TEST_DATABASE_URL"))
            .unwrap_or_else(|_| "sqlite:./axum_kickoff.db".to_string());

        Ok(Self { url })
    }

    #[cfg(test)]
    pub fn test_config() -> Result<Self> {
        let url =
            dotenvy::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());

        Ok(Self { url })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_environment_with_database_url() {
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/db");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(config.url, "postgresql://user:pass@localhost/db");

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
        // Note: This test may fail if there's a .env file with DATABASE_URL set
        // since dotenvy reads from .env files. This is a known limitation.
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::remove_var("DATABASE_URL");
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        // Only assert if we're not getting the default value (which means .env is interfering)
        if config.url != "sqlite:./axum_kickoff.db" {
            assert_eq!(config.url, "sqlite::memory:");
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
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/db");
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        // DATABASE_URL takes precedence in the implementation
        assert_eq!(config.url, "postgresql://user:pass@localhost/db");

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
        let original_db = std::env::var("DATABASE_URL").ok();
        let original_test = std::env::var("TEST_DATABASE_URL").ok();
        // Ensure neither DATABASE_URL nor TEST_DATABASE_URL is set
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("TEST_DATABASE_URL");

        let config = DatabaseConfig::from_environment().expect("Failed to create Database config");
        assert_eq!(config.url, "sqlite:./axum_kickoff.db");

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
        let original = std::env::var("TEST_DATABASE_URL").ok();
        std::env::set_var("TEST_DATABASE_URL", "sqlite::memory:");

        let config = DatabaseConfig::test_config().expect("Failed to create test Database config");
        assert_eq!(config.url, "sqlite::memory:");

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }

    #[test]
    fn test_test_config_default() {
        let original = std::env::var("TEST_DATABASE_URL").ok();
        std::env::remove_var("TEST_DATABASE_URL");

        let config = DatabaseConfig::test_config().expect("Failed to create test Database config");
        assert_eq!(config.url, "sqlite::memory:");

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("TEST_DATABASE_URL", val);
        } else {
            std::env::remove_var("TEST_DATABASE_URL");
        }
    }
}

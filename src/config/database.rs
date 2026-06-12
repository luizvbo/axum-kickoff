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
        let url = dotenvy::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite::memory:".to_string());

        Ok(Self { url })
    }
}

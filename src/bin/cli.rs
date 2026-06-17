//! Toasty migration CLI
//!
//! This binary provides commands for managing database migrations:
//! - migration generate: Generate a new migration based on model changes
//! - migration apply: Apply pending migrations
//! - migration snapshot: Create a schema snapshot
//! - migration drop: Drop the last migration
//! - migration reset: Reset the database (drop all tables and reapply migrations)

use anyhow::Result;
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;

    // Load database configuration from environment
    let db_config = axum_kickoff::config::DatabaseConfig::from_environment()?;

    let db = toasty::Db::builder()
        .models(toasty::models!(axum_kickoff::*))
        .connect(&db_config.url)
        .await?;

    let cli = ToastyCli::with_config(db, config);
    cli.parse_and_run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: CLI binary tests are typically integration tests
    // Unit tests for binary entry points are limited
    // Consider adding integration tests in tests/ directory

    #[test]
    fn test_placeholder() {
        // Placeholder to indicate test module exists
        // Real CLI testing requires integration tests with subprocess execution
        assert!(true);
    }
}

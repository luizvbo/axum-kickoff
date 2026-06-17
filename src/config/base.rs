//! Base configuration options
//!
//! - `HEROKU`: Is this instance of the app currently running on Heroku.

use crate::Env;

#[derive(Clone)]
pub struct Base {
    pub env: Env,
}

impl Base {
    pub fn from_environment() -> anyhow::Result<Self> {
        let env = match std::env::var("HEROKU").as_deref() {
            Ok(_) => Env::Production,
            _ => Env::Development,
        };

        Ok(Self { env })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_environment_development() {
        // Save and restore the original value
        let original = std::env::var("HEROKU").ok();
        std::env::remove_var("HEROKU");

        let base = Base::from_environment().expect("Failed to create Base config");
        assert_eq!(base.env, Env::Development);

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("HEROKU", val);
        }
    }

    #[test]
    fn test_from_environment_production() {
        // Save and restore the original value
        let original = std::env::var("HEROKU").ok();
        std::env::set_var("HEROKU", "true");

        let base = Base::from_environment().expect("Failed to create Base config");
        assert_eq!(base.env, Env::Production);

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("HEROKU", val);
        } else {
            std::env::remove_var("HEROKU");
        }
    }
}

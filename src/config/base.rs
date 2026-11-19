//! Base configuration options
//!
//! - `APP_ENV`: The environment the application is running in. May be
//!   `development`, `test`, or `production`. Defaults to `production` when
//!   unset so that a deployment missing the variable fails secure — set
//!   `APP_ENV=development` for local development.

use crate::Env;

#[derive(Clone)]
pub struct Base {
    pub env: Env,
}

impl Base {
    pub fn from_environment() -> anyhow::Result<Self> {
        let env = match crate::config::env::var("APP_ENV")? {
            Some(value) => match value.to_lowercase().as_str() {
                "development" => Env::Development,
                "test" => Env::Test,
                "production" => Env::Production,
                _ => anyhow::bail!("APP_ENV must be `development`, `test`, or `production`"),
            },
            // Secure-by-default: an unset APP_ENV must not silently enable
            // development behavior (debug routes, non-Secure cookies, no
            // HSTS, pretty logs).
            None => Env::Production,
        };

        Ok(Self { env })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Serialize tests that mutate process environment variables.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn remove_app_env() {
        std::env::remove_var("APP_ENV");
    }

    fn restore_app_env(app_env: Option<String>) {
        if let Some(val) = app_env {
            std::env::set_var("APP_ENV", val);
        } else {
            std::env::remove_var("APP_ENV");
        }
    }

    #[test]
    fn test_from_environment_defaults_to_production() {
        let _guard = ENV_LOCK.lock();
        let original_app_env = std::env::var("APP_ENV").ok();
        remove_app_env();

        let base = Base::from_environment().expect("Failed to create Base config");
        assert_eq!(base.env, Env::Production);

        restore_app_env(original_app_env);
    }

    #[test]
    fn test_from_environment_test() {
        let _guard = ENV_LOCK.lock();
        let original_app_env = std::env::var("APP_ENV").ok();
        remove_app_env();
        std::env::set_var("APP_ENV", "test");

        let base = Base::from_environment().expect("Failed to create Base config");
        assert_eq!(base.env, Env::Test);

        restore_app_env(original_app_env);
    }

    #[test]
    fn test_from_environment_production_from_app_env() {
        let _guard = ENV_LOCK.lock();
        let original_app_env = std::env::var("APP_ENV").ok();
        remove_app_env();
        std::env::set_var("APP_ENV", "production");

        let base = Base::from_environment().expect("Failed to create Base config");
        assert_eq!(base.env, Env::Production);

        restore_app_env(original_app_env);
    }
}

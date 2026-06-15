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
        let env = match dotenvy::var("HEROKU").as_deref() {
            Ok(_) => Env::Production,
            _ => Env::Development,
        };

        Ok(Self { env })
    }
}

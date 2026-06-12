//! Base configuration options

use crate::Env;

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

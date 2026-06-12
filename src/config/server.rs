//! Server configuration

use crate::Env;
use cookie::Key;

pub struct Server {
    pub env: Env,
    pub session_key: Key,
}

impl Server {
    pub fn from_env() -> anyhow::Result<Self> {
        let base = super::Base::from_environment()?;
        
        // Generate a session key (in production, this should come from config)
        let session_key = Key::generate();

        Ok(Self {
            env: base.env,
            session_key,
        })
    }

    pub fn env(&self) -> Env {
        self.env
    }
}

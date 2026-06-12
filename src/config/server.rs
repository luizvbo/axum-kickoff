//! Server configuration
//!
//! Pulls values from the following environment variables:
//!
//! - `SESSION_KEY`: The key used to sign and encrypt session cookies (required).
//! - `PORT`: The port to listen on (defaults to 8888).
//! - `DEV_DOCKER`: Set to any value to indicate running in Docker (defaults to 127.0.0.1 bind).
//! - `HEROKU`: Set to any value to indicate running on Heroku (defaults to 0.0.0.0 bind).
//! - `SERVER_THREADS`: Maximum number of blocking threads (optional).
//! - `DOMAIN_NAME`: The domain name of the application (defaults to "localhost").
//! - `WEB_ALLOWED_ORIGINS`: Comma-separated list of allowed CORS origins (required).
//! - `BLOCKED_IPS`: Comma-separated list of blocked IP addresses (optional).
//! - `BLOCKED_ROUTES`: Comma-separated list of blocked route patterns (optional).
//! - `BLOCKED_TRAFFIC`: Comma-separated list of header=value pairs for blocking traffic (optional).

use crate::Env;
use crate::middleware::block_traffic::BlockCriteria;
use http::HeaderValue;
use std::collections::HashSet;
use std::net::IpAddr;
use std::str::FromStr;

use super::base::Base;

pub struct Server {
    pub base: Base,
    pub ip: IpAddr,
    pub port: u16,
    pub max_blocking_threads: Option<usize>,
    pub domain_name: String,
    pub allowed_origins: AllowedOrigins,
    pub blocked_ips: HashSet<IpAddr>,
    pub blocked_routes: HashSet<String>,
    pub blocked_traffic: Vec<(String, Vec<BlockCriteria>)>,
    pub session_key: cookie::Key,
}

impl Server {
    /// Returns a default value for the application's config
    ///
    /// # Panics
    ///
    /// This function panics if the Server configuration is invalid.
    pub fn from_environment() -> anyhow::Result<Self> {
        let docker = dotenvy::var("DEV_DOCKER").is_ok();
        let heroku = dotenvy::var("HEROKU").is_ok();

        let ip = if heroku || docker {
            [0, 0, 0, 0].into()
        } else {
            [127, 0, 0, 1].into()
        };

        let port = dotenvy::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8888);
        
        let max_blocking_threads = dotenvy::var("SERVER_THREADS")
            .ok()
            .and_then(|s| s.parse().ok());
        
        let base = Base::from_environment()?;
        
        let domain_name = dotenvy::var("DOMAIN_NAME")
            .unwrap_or_else(|_| "localhost".into());
        
        let allowed_origins = AllowedOrigins::from_default_env()?;

        // Parse blocked IPs
        let blocked_ips: HashSet<IpAddr> = dotenvy::var("BLOCKED_IPS")
            .ok()
            .and_then(|s| {
                s.split(',')
                    .map(|ip| ip.trim().parse::<IpAddr>())
                    .collect::<Result<HashSet<_>, _>>()
                    .ok()
            })
            .unwrap_or_default();

        // Parse blocked routes
        let blocked_routes: HashSet<String> = dotenvy::var("BLOCKED_ROUTES")
            .ok()
            .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
            .unwrap_or_default();

        // Parse blocked traffic (header=value pairs)
        let blocked_traffic = parse_blocked_traffic()?;

        // Load session key for signing cookies
        let session_key = dotenvy::var("SESSION_KEY")
            .map_err(|_| {
                tracing::error!("Required environment variable 'SESSION_KEY' is not set");
                anyhow::anyhow!("Required environment variable 'SESSION_KEY' is not set")
            })?;
        let session_key = cookie::Key::try_from(session_key.as_bytes())
            .map_err(|e| {
                tracing::error!("Invalid SESSION_KEY: {}. The key must be at least 32 bytes long.", e);
                anyhow::anyhow!("Invalid SESSION_KEY: {}. The key must be at least 32 bytes long.", e)
            })?;

        Ok(Server {
            base,
            ip,
            port,
            max_blocking_threads,
            domain_name,
            allowed_origins,
            blocked_ips,
            blocked_routes,
            blocked_traffic,
            session_key,
        })
    }

    pub fn env(&self) -> Env {
        self.base.env
    }
}

/// Parse BLOCKED_TRAFFIC environment variable
///
/// Format: "Header1=ENV_VAR1,Header2=ENV_VAR2"
/// Each ENV_VAR should contain comma-separated values to block
fn parse_blocked_traffic() -> anyhow::Result<Vec<(String, Vec<BlockCriteria>)>> {
    let blocked_traffic_str = match dotenvy::var("BLOCKED_TRAFFIC") {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let mut result = Vec::new();

    for pair in blocked_traffic_str.split(',') {
        let pair = pair.trim();
        let parts: Vec<&str> = pair.split('=').collect();
        
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid BLOCKED_TRAFFIC format: {}", pair));
        }

        let header_name = parts[0].trim().to_string();
        let env_var_name = parts[1].trim();
        
        let env_value = dotenvy::var(env_var_name)
            .map_err(|_| anyhow::anyhow!("Environment variable {} not found", env_var_name))?;
        
        let blocked_values: Vec<BlockCriteria> = env_value
            .split(',')
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| BlockCriteria::try_from(v))
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow::anyhow!("Invalid block criteria: {}", e))?;

        if !blocked_values.is_empty() {
            result.push((header_name, blocked_values));
        }
    }

    Ok(result)
}

#[derive(Clone, Debug, Default)]
pub struct AllowedOrigins(Vec<String>);

impl AllowedOrigins {
    pub fn from_str(s: &str) -> Self {
        Self(s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
    }

    pub fn from_default_env() -> anyhow::Result<Self> {
        let value = dotenvy::var("WEB_ALLOWED_ORIGINS")
            .map_err(|_| anyhow::anyhow!("Required environment variable 'WEB_ALLOWED_ORIGINS' is not set"))?;
        Ok(Self::from_str(&value))
    }

    pub fn contains(&self, value: &HeaderValue) -> bool {
        self.0.iter().any(|it| it == value)
    }
}

impl FromStr for AllowedOrigins {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_str(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_origins_from_str() {
        let origins = AllowedOrigins::from_str("http://localhost:3000,https://example.com");
        assert_eq!(origins.0, vec!["http://localhost:3000", "https://example.com"]);
    }

    #[test]
    fn test_allowed_origins_trim_whitespace() {
        let origins = AllowedOrigins::from_str(" http://localhost:3000 , https://example.com ");
        assert_eq!(origins.0, vec!["http://localhost:3000", "https://example.com"]);
    }

    #[test]
    fn test_allowed_origins_empty_values() {
        let origins = AllowedOrigins::from_str("http://localhost:3000,,https://example.com");
        assert_eq!(origins.0, vec!["http://localhost:3000", "https://example.com"]);
    }
}

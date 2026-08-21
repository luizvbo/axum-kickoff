//! Server configuration
//!
//! Pulls values from the following environment variables:
//!
//! - `SESSION_KEY`: The key used to sign and encrypt session cookies (required).
//! - `PORT`: The port to listen on (defaults to 8888).
//! - `DEV_DOCKER`: Set to any value to indicate running in Docker (defaults to 127.0.0.1 bind).
//! - `HEROKU`: Set to any value to indicate running on Heroku (defaults to 0.0.0.0 bind).
//! - `APP_ENV`: The environment the application is running in (`development`, `test`,
//!   or `production`).
//! - `SERVER_THREADS`: Maximum number of blocking threads (optional).
//! - `DOMAIN_NAME`: The domain name of the application (defaults to "localhost").
//! - `WEB_ALLOWED_ORIGINS`: Comma-separated list of allowed CORS origins (required).
//! - `BLOCKED_IPS`: Comma-separated list of blocked IP addresses (optional).
//! - `BLOCKED_ROUTES`: Comma-separated list of blocked route patterns (optional).
//! - `BLOCKED_TRAFFIC`: Comma-separated list of header=value pairs for blocking traffic (optional).
//! - `GH_CLIENT_ID`: GitHub OAuth client ID (required for OAuth).
//! - `GH_CLIENT_SECRET`: GitHub OAuth client secret (required for OAuth).
//! - `GH_REDIRECT_URI`: GitHub OAuth redirect URI (defaults to "https://`<domain>`:`<port>`/api/v1/auth/github/callback" in production, "http://" in development).
//! - `STORAGE_PATH`: Path for local filesystem storage (defaults to "./local_uploads").
//! - `CDN_PREFIX`: Optional CDN prefix for generating public URLs.
//! - `TRUSTED_PROXIES`: Comma-separated list of trusted proxy IPs/CIDR ranges (defaults to "127.0.0.1,::1").
//! - `METRICS_TOKEN`: Optional token for accessing the metrics endpoint.
//! - `SENTRY_DSN`: Optional Sentry DSN.
//! - `LOG_FORMAT`: Optional log format override (`pretty`, `full`, `compact`, or `json`).

use crate::middleware::block_traffic::BlockCriteria;
use crate::rate_limiter::{LimitedAction, RateLimiterConfig};
use crate::storage::StorageConfig;
use crate::Env;
use anyhow::Context;
use http::HeaderValue;
use secrecy::{ExposeSecret, SecretString};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

use super::base::Base;
use super::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// Human-readable, multi-line output. Default for development and test.
    #[default]
    Pretty,
    /// Default single-line output.
    Full,
    /// Shorter single-line output.
    Compact,
    /// JSON output. Default for production.
    Json,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pretty" => Ok(LogFormat::Pretty),
            "full" => Ok(LogFormat::Full),
            "compact" => Ok(LogFormat::Compact),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("Invalid log format: {s}")),
        }
    }
}

fn default_log_format(env: Env) -> LogFormat {
    if env == Env::Production {
        LogFormat::Json
    } else {
        LogFormat::Pretty
    }
}

#[derive(Clone)]
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
    pub session_key: SecretString,
    pub trusted_proxies: Vec<ipnet::IpNet>,
    pub gh_client_id: String,
    pub gh_client_secret: SecretString,
    pub gh_redirect_uri: String,
    pub storage_config: StorageConfig,
    pub rate_limiter_config: HashMap<LimitedAction, RateLimiterConfig>,
    pub metrics_token: Option<SecretString>,
    pub sentry_dsn: Option<SecretString>,
    pub log_format: LogFormat,
}

impl Server {
    /// Returns a default value for the application's config
    ///
    /// # Panics
    ///
    /// This function panics if the Server configuration is invalid.
    pub fn from_environment() -> anyhow::Result<Self> {
        let docker = env::var("DEV_DOCKER")?.is_some();
        let heroku = env::var("HEROKU")?.is_some();

        let ip = if heroku || docker {
            [0, 0, 0, 0].into()
        } else {
            [127, 0, 0, 1].into()
        };

        let port = env::var_parsed("PORT")?.unwrap_or(8888);
        let max_blocking_threads = env::var_parsed("SERVER_THREADS")?;

        let base = Base::from_environment()?;

        let domain_name = env::var("DOMAIN_NAME")?.unwrap_or_else(|| "localhost".into());

        let allowed_origins = AllowedOrigins::from_default_env()?;

        // Parse blocked IPs
        let blocked_ips: HashSet<IpAddr> = env::var("BLOCKED_IPS")?
            .and_then(|s| {
                s.split(',')
                    .map(|ip| ip.trim().parse::<IpAddr>())
                    .collect::<Result<HashSet<_>, _>>()
                    .ok()
            })
            .unwrap_or_default();

        // Parse blocked routes
        let blocked_routes: HashSet<String> = env::var("BLOCKED_ROUTES")?
            .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
            .unwrap_or_default();

        // Parse blocked traffic (header=value pairs)
        let blocked_traffic = parse_blocked_traffic_from_env()?;

        // Load session key for signing cookies
        let session_key = SecretString::from(env::required_var("SESSION_KEY")?);

        // Load GitHub OAuth credentials
        let gh_client_id = env::required_var("GH_CLIENT_ID")?;
        let gh_client_secret = SecretString::from(env::required_var("GH_CLIENT_SECRET")?);
        let gh_redirect_uri = env::var("GH_REDIRECT_URI")?.unwrap_or_else(|| {
            let scheme = if base.env == Env::Production {
                "https"
            } else {
                "http"
            };
            format!(
                "{}://{}:{}/api/v1/auth/github/callback",
                scheme, domain_name, port
            )
        });

        // Load storage configuration
        let storage_config = StorageConfig::from_environment();

        // Parse trusted proxies (default to localhost for safety)
        let trusted_proxies = parse_trusted_proxies()?;

        // Parse rate limiter configuration from environment
        let rate_limiter_config = parse_rate_limiter_config()?;

        let metrics_token = env::var("METRICS_TOKEN")?.map(SecretString::from);
        let sentry_dsn = env::var("SENTRY_DSN")?.map(SecretString::from);

        let log_format = match env::var("LOG_FORMAT")? {
            Some(value) => value
                .parse()
                .unwrap_or_else(|_| default_log_format(base.env)),
            None => default_log_format(base.env),
        };

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
            trusted_proxies,
            gh_client_id,
            gh_client_secret,
            gh_redirect_uri,
            storage_config,
            rate_limiter_config,
            metrics_token,
            sentry_dsn,
            log_format,
        })
    }

    pub fn env(&self) -> Env {
        self.base.env
    }

    pub fn cookie_key(&self) -> cookie::Key {
        cookie::Key::derive_from(self.session_key.expose_secret().as_bytes())
    }

    pub fn sentry_enabled(&self) -> bool {
        self.sentry_dsn.is_some() && self.base.env == Env::Production
    }
}

/// Parse TRUSTED_PROXIES environment variable
///
/// Format: "127.0.0.1,::1,10.0.0.0/8"
/// Defaults to "127.0.0.1/32,::1/128" (localhost) for safety
fn parse_trusted_proxies() -> anyhow::Result<Vec<ipnet::IpNet>> {
    let trusted_proxies_str =
        env::var("TRUSTED_PROXIES")?.unwrap_or_else(|| "127.0.0.1/32,::1/128".to_string());

    let mut result = Vec::new();

    for entry in trusted_proxies_str.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let ipnet: ipnet::IpNet = entry
            .parse()
            .with_context(|| format!("Invalid trusted proxy entry '{entry}'"))?;

        result.push(ipnet);
    }

    if result.is_empty() {
        // Fallback to localhost if parsing resulted in empty list
        result.push("127.0.0.1".parse().unwrap());
        result.push("::1".parse().unwrap());
    }

    Ok(result)
}

/// Parse RATE_LIMITER_* environment variables
///
/// Variables are of the form `RATE_LIMITER_<ACTION>_RATE_SECONDS` and
/// `RATE_LIMITER_<ACTION>_BURST`. If not present, defaults for the action are used.
fn parse_rate_limiter_config() -> anyhow::Result<HashMap<LimitedAction, RateLimiterConfig>> {
    let mut config = HashMap::new();

    for action in LimitedAction::VARIANTS {
        let key = action.env_var_key();
        let rate = env::var_parsed::<u64>(&format!("RATE_LIMITER_{key}_RATE_SECONDS"))?
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(action.default_rate_seconds()));

        let burst = env::var_parsed::<i32>(&format!("RATE_LIMITER_{key}_BURST"))?
            .unwrap_or(action.default_burst());

        config.insert(action, RateLimiterConfig { rate, burst });
    }

    Ok(config)
}

/// Parse the `BLOCKED_TRAFFIC` value from the environment.
///
/// Format: "Header1=ENV_VAR1,Header2=ENV_VAR2"
/// Each ENV_VAR should contain comma-separated values to block.
fn parse_blocked_traffic_from_env() -> anyhow::Result<Vec<(String, Vec<BlockCriteria>)>> {
    let blocked_traffic_str = env::var("BLOCKED_TRAFFIC")?;
    parse_blocked_traffic(blocked_traffic_str.as_deref(), env::var)
}

/// Parse a `BLOCKED_TRAFFIC` value and resolve referenced value variables
/// using the provided `getenv` callback.
///
/// `getenv` is called for each environment variable named on the right-hand
/// side of a `Header=ENV_VAR` pair. In production it reads the process
/// environment; in tests it can be replaced with a stub so that no global
/// state is mutated.
fn parse_blocked_traffic<F>(
    blocked_traffic_str: Option<&str>,
    getenv: F,
) -> anyhow::Result<Vec<(String, Vec<BlockCriteria>)>>
where
    F: Fn(&str) -> anyhow::Result<Option<String>>,
{
    let blocked_traffic_str = match blocked_traffic_str {
        Some(s) if s.trim().is_empty() => return Ok(Vec::new()),
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let mut result = Vec::new();

    for pair in blocked_traffic_str.split(',') {
        let pair = pair.trim();
        let parts: Vec<&str> = pair.split('=').collect();

        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid BLOCKED_TRAFFIC format: {pair}"));
        }

        let header_name = parts[0].trim().to_string();
        let env_var_name = parts[1].trim();

        let env_value = getenv(env_var_name)?
            .with_context(|| format!("Environment variable {env_var_name} not found"))?;

        let blocked_values: Vec<BlockCriteria> = env_value
            .split(',')
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(BlockCriteria::try_from)
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow::anyhow!("Invalid block criteria: {e}"))?;

        if !blocked_values.is_empty() {
            result.push((header_name, blocked_values));
        }
    }

    Ok(result)
}

#[derive(Clone, Debug, Default)]
pub struct AllowedOrigins(Vec<String>);

impl AllowedOrigins {
    pub fn parse(s: &str) -> Self {
        Self(
            s.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    pub fn from_default_env() -> anyhow::Result<Self> {
        let value = env::required_var("WEB_ALLOWED_ORIGINS")?;
        Ok(Self::parse(&value))
    }

    pub fn contains(&self, value: &HeaderValue) -> bool {
        self.0.iter().any(|it| it == value)
    }

    pub fn origins(&self) -> &[String] {
        &self.0
    }
}

impl FromStr for AllowedOrigins {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use std::collections::HashMap;

    fn getenv_stub<'a>(
        values: &'a HashMap<&str, &str>,
    ) -> impl Fn(&str) -> anyhow::Result<Option<String>> + 'a {
        move |name| Ok(values.get(name).map(|s| s.to_string()))
    }

    #[test]
    fn test_allowed_origins_from_str() {
        let origins = AllowedOrigins::parse("http://localhost:3000,https://example.com");
        assert_eq!(
            origins.0,
            vec!["http://localhost:3000", "https://example.com"]
        );
    }

    #[test]
    fn test_allowed_origins_trim_whitespace() {
        let origins = AllowedOrigins::parse(" http://localhost:3000 , https://example.com ");
        assert_eq!(
            origins.0,
            vec!["http://localhost:3000", "https://example.com"]
        );
    }

    #[test]
    fn test_allowed_origins_empty_values() {
        let origins = AllowedOrigins::parse("http://localhost:3000,,https://example.com");
        assert_eq!(
            origins.0,
            vec!["http://localhost:3000", "https://example.com"]
        );
    }

    #[test]
    fn test_allowed_origins_contains() {
        let origins = AllowedOrigins::parse("http://localhost:3000,https://example.com");
        let header = HeaderValue::from_static("http://localhost:3000");
        assert!(origins.contains(&header));
    }

    #[test]
    fn test_allowed_origins_not_contains() {
        let origins = AllowedOrigins::parse("http://localhost:3000,https://example.com");
        let header = HeaderValue::from_static("http://other.com");
        assert!(!origins.contains(&header));
    }

    #[test]
    fn test_allowed_origins_origins() {
        let origins = AllowedOrigins::parse("http://localhost:3000,https://example.com");
        assert_eq!(origins.origins().len(), 2);
        assert_eq!(origins.origins()[0], "http://localhost:3000");
    }

    #[test]
    fn test_allowed_origins_from_str_trait() {
        let origins: AllowedOrigins = "http://localhost:3000".parse().unwrap();
        assert_eq!(origins.0, vec!["http://localhost:3000"]);
    }

    #[test]
    fn test_allowed_origins_default() {
        let origins = AllowedOrigins::default();
        assert!(origins.0.is_empty());
    }

    #[test]
    fn test_parse_blocked_traffic_empty() {
        let values = HashMap::new();
        let result = parse_blocked_traffic(None, getenv_stub(&values));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_blocked_traffic_invalid_format() {
        let values = HashMap::new();
        let result = parse_blocked_traffic(Some("invalid_format"), getenv_stub(&values));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_blocked_traffic_missing_env_var() {
        let values = HashMap::new();
        let result = parse_blocked_traffic(Some("Header=MISSING_VAR"), getenv_stub(&values));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_blocked_traffic_valid() {
        let mut values = HashMap::new();
        values.insert("BLOCKED_AGENTS", "bot1,bot2");
        let result = parse_blocked_traffic(Some("User-Agent=BLOCKED_AGENTS"), getenv_stub(&values));
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "User-Agent");
        assert_eq!(parsed[0].1.len(), 2);
    }

    #[test]
    fn test_parse_blocked_traffic_empty_values() {
        let mut values = HashMap::new();
        values.insert("BLOCKED_VALUES", ",,");
        let result = parse_blocked_traffic(Some("Header=BLOCKED_VALUES"), getenv_stub(&values));
        assert!(result.is_ok());
        let parsed = result.unwrap();
        // Empty values should be filtered out
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_allowed_origins_single() {
        let origins = AllowedOrigins::parse("http://localhost:3000");
        assert_eq!(origins.0, vec!["http://localhost:3000"]);
    }

    #[test]
    fn test_allowed_origins_empty_string() {
        let origins = AllowedOrigins::parse("");
        assert!(origins.0.is_empty());
    }

    #[test]
    fn test_allowed_origins_only_whitespace() {
        let origins = AllowedOrigins::parse("   ,   ,   ");
        assert!(origins.0.is_empty());
    }

    #[test]
    fn test_allowed_origins_multiple_commas() {
        let origins = AllowedOrigins::parse("http://localhost:3000,,,https://example.com");
        assert_eq!(
            origins.0,
            vec!["http://localhost:3000", "https://example.com"]
        );
    }

    #[test]
    fn test_allowed_origins_contains_case_sensitive() {
        let origins = AllowedOrigins::parse("http://localhost:3000");
        let header = HeaderValue::from_static("http://localhost:3000");
        assert!(origins.contains(&header));

        let header_upper = HeaderValue::from_static("HTTP://LOCALHOST:3000");
        assert!(!origins.contains(&header_upper));
    }

    #[test]
    fn test_allowed_origins_clone() {
        let origins = AllowedOrigins::parse("http://localhost:3000");
        let cloned = origins.clone();
        assert_eq!(origins.0, cloned.0);
    }

    #[test]
    fn test_allowed_origins_debug() {
        let origins = AllowedOrigins::parse("http://localhost:3000");
        let debug_str = format!("{:?}", origins);
        assert!(debug_str.contains("localhost"));
    }

    #[test]
    fn test_parse_blocked_traffic_multiple_pairs() {
        let mut values = HashMap::new();
        values.insert("BLOCKED_AGENTS", "bot1,bot2");
        values.insert("BLOCKED_REFERRERS", "spam1,spam2");
        let result = parse_blocked_traffic(
            Some("User-Agent=BLOCKED_AGENTS,Referer=BLOCKED_REFERRERS"),
            getenv_stub(&values),
        );
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn test_parse_blocked_traffic_whitespace_in_pairs() {
        let mut values = HashMap::new();
        values.insert("BLOCKED_AGENTS", "bot1");
        let result =
            parse_blocked_traffic(Some(" User-Agent = BLOCKED_AGENTS "), getenv_stub(&values));
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed[0].0, "User-Agent");
    }

    #[test]
    fn test_allowed_origins_from_str_empty() {
        let origins: AllowedOrigins = "".parse().unwrap();
        assert!(origins.0.is_empty());
    }

    #[test]
    fn test_allowed_origins_origins_immutable() {
        let origins = AllowedOrigins::parse("http://localhost:3000");
        let slice = origins.origins();
        assert_eq!(slice.len(), 1);
        // Verify we get a reference, not ownership
        let _ = &slice[0];
    }
}

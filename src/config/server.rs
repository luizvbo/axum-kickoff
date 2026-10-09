//! Server configuration
//!
//! Pulls values from the following environment variables:
//!
//! - `SESSION_KEY`: The key used to sign and encrypt session cookies (required).
//! - `PORT`: The port to listen on (defaults to 8888).
//! - `DEV_DOCKER`: Set to any value to indicate running in Docker (defaults to 127.0.0.1 bind).
//! - `HEROKU`: Set to any value to indicate running on Heroku (defaults to 0.0.0.0 bind).
//! - `APP_ENV`: The environment the application is running in (`development`, `test`,
//!   or `production`). Defaults to `production` when unset.
//! - `SERVER_THREADS`: Maximum number of blocking threads (`max_blocking_threads`,
//!   the `spawn_blocking` pool — optional).
//! - `SERVER_CORE_THREADS`: Number of async worker threads (`worker_threads` —
//!   optional, defaults to the available CPU parallelism).
//! - `DOMAIN_NAME`: The domain name of the application (defaults to "localhost").
//! - `WEB_ALLOWED_ORIGINS`: Comma-separated list of allowed CORS origins (required).
//! - `BLOCKED_IPS`: Comma-separated list of blocked IP addresses (optional).
//! - `BLOCKED_ROUTES`: Comma-separated list of blocked route patterns (optional).
//! - `BLOCKED_TRAFFIC`: Comma-separated list of header=value pairs for blocking traffic (optional).
//! - OAuth provider credentials: for each compiled-in provider
//!   (`crate::oauth::provider_specs`), setting both `<PREFIX>_CLIENT_ID` and
//!   `<PREFIX>_CLIENT_SECRET` enables sign-in with that provider (e.g.
//!   `GH_CLIENT_ID`/`GH_CLIENT_SECRET` for GitHub, `GOOGLE_*` for Google,
//!   `FACEBOOK_*` for Facebook). Setting only one of the pair is a startup
//!   error. `<PREFIX>_REDIRECT_URI` optionally overrides the callback URL,
//!   which defaults to "https://`<domain>`:`<port>`/api/v1/auth/`<provider>`/callback"
//!   in production, "http://" in development.
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
    /// `SERVER_THREADS`: cap on the blocking thread pool (`spawn_blocking`).
    pub max_blocking_threads: Option<usize>,
    /// `SERVER_CORE_THREADS`: async worker threads; `None` = available
    /// parallelism (Tokio's default).
    pub core_threads: Option<usize>,
    pub domain_name: String,
    pub allowed_origins: AllowedOrigins,
    pub blocked_ips: HashSet<IpAddr>,
    pub blocked_routes: HashSet<String>,
    pub blocked_traffic: Vec<(String, Vec<BlockCriteria>)>,
    pub session_key: SecretString,
    pub trusted_proxies: Vec<ipnet::IpNet>,
    /// OAuth providers enabled by present credentials — see
    /// [`Self::oauth_provider`].
    pub oauth_providers: Vec<OAuthProviderConfig>,
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
        let core_threads = env::var_parsed("SERVER_CORE_THREADS")?;

        let base = Base::from_environment()?;

        let domain_name = env::var("DOMAIN_NAME")?.unwrap_or_else(|| "localhost".into());

        let allowed_origins = AllowedOrigins::from_default_env()?;

        // Parse blocked IPs. A malformed entry is a startup error: the
        // previous `.ok()` discarded the *entire* list on any parse failure,
        // silently disabling IP blocking while the operator believed it was
        // active (fail-open). Fail fast instead, like TRUSTED_PROXIES and
        // BLOCKED_TRAFFIC do.
        let blocked_ips = parse_blocked_ips(env::var("BLOCKED_IPS")?)?;

        // Parse blocked routes
        let blocked_routes: HashSet<String> = env::var("BLOCKED_ROUTES")?
            .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
            .unwrap_or_default();

        // Parse blocked traffic (header=value pairs)
        let blocked_traffic = parse_blocked_traffic_from_env()?;

        // Load session key for signing cookies
        let session_key = SecretString::from(env::required_var("SESSION_KEY")?);

        // `cookie::Key::derive_from` accepts keys of any length, so enforce
        // the documented 64-byte minimum explicitly in production rather
        // than letting a weak key sign real sessions.
        if base.env == Env::Production {
            let key = session_key.expose_secret();
            if key.len() < 64 {
                anyhow::bail!("SESSION_KEY must be at least 64 bytes when APP_ENV=production");
            }
            // The `.env.sample` placeholder is long enough to pass the length
            // check but is publicly known — a verbatim copy to `.env` would
            // let anyone forge session cookies.
            if key == SAMPLE_SESSION_KEY {
                anyhow::bail!(
                    "SESSION_KEY still contains the .env.sample placeholder; \
                     generate a random key (e.g. `openssl rand -base64 48`)"
                );
            }
        }

        // Load OAuth provider credentials. A compiled-in provider is enabled
        // when both its client ID and secret are set; supplying only one is a
        // configuration error (fail fast instead of silently disabling
        // sign-in).
        let mut oauth_providers = Vec::new();
        for spec in crate::oauth::provider_specs() {
            // Empty/whitespace values count as unset: `VAR=` would otherwise
            // enable the provider with empty credentials and only fail at
            // the first real OAuth exchange.
            let client_id = env::var(spec.client_id_env)?.filter(|v| !v.trim().is_empty());
            let client_secret = env::var(spec.client_secret_env)?.filter(|v| !v.trim().is_empty());

            match (client_id, client_secret) {
                (Some(client_id), Some(client_secret)) => {
                    let redirect_uri = env::var(spec.redirect_uri_env)?.unwrap_or_else(|| {
                        let scheme = if base.env == Env::Production {
                            "https"
                        } else {
                            "http"
                        };
                        format!(
                            "{}://{}:{}/api/v1/auth/{}/callback",
                            scheme, domain_name, port, spec.slug
                        )
                    });
                    oauth_providers.push(OAuthProviderConfig {
                        spec,
                        client_id,
                        client_secret: SecretString::from(client_secret),
                        redirect_uri,
                    });
                }
                (None, None) => {}
                _ => anyhow::bail!(
                    "{} and {} must be set together",
                    spec.client_id_env,
                    spec.client_secret_env
                ),
            }
        }

        // Load storage configuration
        let storage_config = StorageConfig::from_environment();

        // Parse trusted proxies (default to localhost for safety)
        let trusted_proxies = parse_trusted_proxies()?;

        // Parse rate limiter configuration from environment
        let rate_limiter_config = parse_rate_limiter_config()?;

        // An empty `METRICS_TOKEN` would require the literal header
        // `Authorization: Bearer ` — a sham protection that appears
        // configured but lets anyone through.
        let metrics_token = env::var("METRICS_TOKEN")?
            .map(|t| require_non_empty("METRICS_TOKEN", t))
            .transpose()?
            .map(SecretString::from);
        let sentry_dsn = env::var("SENTRY_DSN")?
            .map(|t| require_non_empty("SENTRY_DSN", t))
            .transpose()?
            .map(SecretString::from);

        // Fail fast on a typo like `LOG_FORMAT=jsno` instead of silently
        // falling back — consistent with every other parsed variable.
        let log_format = match env::var("LOG_FORMAT")? {
            Some(value) => value.parse().map_err(anyhow::Error::msg)?,
            None => default_log_format(base.env),
        };

        Ok(Server {
            base,
            ip,
            port,
            max_blocking_threads,
            core_threads,
            domain_name,
            allowed_origins,
            blocked_ips,
            blocked_routes,
            blocked_traffic,
            session_key,
            trusted_proxies,
            oauth_providers,
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

    /// Returns the configured OAuth provider matching `slug`
    /// (`"github"`, `"google"`, `"facebook"`), if it was compiled in *and* has
    /// credentials in the environment.
    pub fn oauth_provider(&self, slug: &str) -> Option<&OAuthProviderConfig> {
        self.oauth_providers
            .iter()
            .find(|provider| provider.spec.slug == slug)
    }
}

/// Runtime credentials for one OAuth provider.
///
/// The provider's endpoints, scopes, and env-var names live in its
/// [`crate::oauth::OAuthProviderSpec`]; a config entry exists only when both
/// credentials were present at startup.
#[derive(Clone)]
pub struct OAuthProviderConfig {
    /// The compiled-in provider description this config belongs to.
    pub spec: &'static crate::oauth::OAuthProviderSpec,
    pub client_id: String,
    pub client_secret: SecretString,
    /// The callback URL registered with the provider
    /// (`/api/v1/auth/<slug>/callback` by default).
    pub redirect_uri: String,
}

/// The `SESSION_KEY` placeholder shipped in `.env.sample`. It is 67 bytes —
/// long enough to pass the production length check — so it needs an explicit
/// denylist entry to prevent a verbatim copy from signing real sessions.
const SAMPLE_SESSION_KEY: &str =
    "use-a-session-key-with-64-bytes-or-more-here-and-store-it-securely";

/// Reject environment variables that are set but empty (or whitespace-only).
fn require_non_empty(key: &str, value: String) -> anyhow::Result<String> {
    if value.trim().is_empty() {
        anyhow::bail!("{key} must not be empty when set");
    }
    Ok(value)
}

/// Parse a `BLOCKED_IPS` value into a set of addresses.
///
/// Empty entries (e.g. a trailing comma) are skipped; malformed entries are
/// an error — this is a security control and must fail closed.
fn parse_blocked_ips(raw: Option<String>) -> anyhow::Result<HashSet<IpAddr>> {
    let mut result = HashSet::new();
    let Some(s) = raw else { return Ok(result) };

    for entry in s.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        result.insert(
            entry
                .parse::<IpAddr>()
                .with_context(|| format!("Invalid BLOCKED_IPS entry '{entry}'"))?,
        );
    }

    Ok(result)
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

        let header_name = parts[0].trim();
        // Validate the name eagerly: `HeaderMap::get_all` on an invalid name
        // yields an empty iterator, which would silently disable the rule.
        http::header::HeaderName::from_str(header_name)
            .with_context(|| format!("Invalid BLOCKED_TRAFFIC header name '{header_name}'"))?;
        let header_name = header_name.to_string();
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

    #[test]
    fn test_parse_blocked_ips_valid() {
        let result = parse_blocked_ips(Some("192.168.1.1, 10.0.0.5 ,::1".to_string())).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        assert!(result.contains(&"10.0.0.5".parse::<IpAddr>().unwrap()));
        assert!(result.contains(&"::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_parse_blocked_ips_skips_empty_entries() {
        let result = parse_blocked_ips(Some("10.0.0.1,,10.0.0.2,".to_string())).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_blocked_ips_fails_closed_on_bad_entry() {
        // A single typo must abort startup instead of silently dropping the
        // whole blocklist (the old `.ok()` discarded everything).
        let result = parse_blocked_ips(Some("10.0.0.1,not-an-ip,10.0.0.2".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not-an-ip"));
    }

    #[test]
    fn test_parse_blocked_ips_none_and_empty() {
        assert!(parse_blocked_ips(None).unwrap().is_empty());
        assert!(parse_blocked_ips(Some(String::new())).unwrap().is_empty());
        assert!(parse_blocked_ips(Some(" , ".to_string()))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_parse_blocked_traffic_invalid_header_name() {
        let mut values = HashMap::new();
        values.insert("BLOCKED", "bot");
        // A header name that can never match a real header would otherwise
        // silently disable the rule at request time.
        let result = parse_blocked_traffic(Some("Not A Header!=BLOCKED"), getenv_stub(&values));
        assert!(result.is_err());
    }

    #[test]
    fn test_require_non_empty() {
        assert!(require_non_empty("X", "value".to_string()).is_ok());
        assert!(require_non_empty("X", String::new()).is_err());
        assert!(require_non_empty("X", "   ".to_string()).is_err());
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Snapshot, override, and restore environment variables around `f`.
    /// `None` removes the variable.
    fn with_env(vars: Vec<(&str, Option<&str>)>, f: impl FnOnce()) {
        let originals: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in &vars {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        f();
        for (k, v) in originals {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }

    /// The minimal environment `Server::from_environment` needs, plus every
    /// variable the new tests mutate, reset to a clean state.
    fn base_env() -> Vec<(&'static str, Option<&'static str>)> {
        let mut vars: Vec<(&'static str, Option<&'static str>)> = vec![
            ("APP_ENV", Some("test")),
            ("HEROKU", None),
            ("DEV_DOCKER", None),
            ("SESSION_KEY", Some("test-session-key")),
            ("WEB_ALLOWED_ORIGINS", Some("http://localhost:3000")),
            ("BLOCKED_IPS", None),
            ("BLOCKED_ROUTES", None),
            ("BLOCKED_TRAFFIC", None),
            ("TRUSTED_PROXIES", None),
            ("METRICS_TOKEN", None),
            ("SENTRY_DSN", None),
            ("LOG_FORMAT", None),
        ];
        for spec in crate::oauth::provider_specs() {
            vars.push((spec.client_id_env, None));
            vars.push((spec.client_secret_env, None));
            vars.push((spec.redirect_uri_env, None));
        }
        vars
    }

    #[test]
    fn test_from_environment_rejects_sample_session_key_in_production() {
        let _guard = ENV_LOCK.lock();
        let mut vars = base_env();
        vars.retain(|(k, _)| *k != "APP_ENV");
        vars.push(("APP_ENV", Some("production")));
        vars.retain(|(k, _)| *k != "SESSION_KEY");
        vars.push(("SESSION_KEY", Some(SAMPLE_SESSION_KEY)));
        with_env(vars, || {
            // The .env.sample placeholder is 67 bytes — long enough to pass
            // the length check — but publicly known, so production must
            // reject it outright.
            let err = Server::from_environment().err().unwrap();
            assert!(err.to_string().contains("placeholder"));
        });
    }

    #[test]
    fn test_from_environment_rejects_short_session_key_in_production() {
        let _guard = ENV_LOCK.lock();
        let mut vars = base_env();
        vars.retain(|(k, _)| *k != "APP_ENV");
        vars.push(("APP_ENV", Some("production")));
        vars.retain(|(k, _)| *k != "SESSION_KEY");
        vars.push(("SESSION_KEY", Some("short")));
        with_env(vars, || {
            assert!(Server::from_environment().is_err());
        });
    }

    #[test]
    fn test_from_environment_rejects_empty_metrics_token() {
        let _guard = ENV_LOCK.lock();
        let mut vars = base_env();
        vars.push(("METRICS_TOKEN", Some("")));
        with_env(vars, || {
            // `METRICS_TOKEN=` must abort startup, not produce a sham
            // `Authorization: Bearer ` requirement.
            assert!(Server::from_environment().is_err());
        });
    }

    #[test]
    fn test_from_environment_rejects_invalid_log_format() {
        let _guard = ENV_LOCK.lock();
        let mut vars = base_env();
        vars.push(("LOG_FORMAT", Some("jsno")));
        with_env(vars, || {
            assert!(Server::from_environment().is_err());
        });
    }

    #[test]
    fn test_from_environment_empty_oauth_credentials_not_enabled() {
        let _guard = ENV_LOCK.lock();
        let specs = crate::oauth::provider_specs();
        if specs.is_empty() {
            return;
        }
        let mut vars = base_env();
        // `VAR=` counts as unset: an empty client id + secret must not
        // enable the provider.
        for spec in &specs {
            vars.push((spec.client_id_env, Some("")));
            vars.push((spec.client_secret_env, Some("  ")));
        }
        with_env(vars, || {
            let config = Server::from_environment().unwrap();
            assert!(config.oauth_providers.is_empty());
        });
    }

    #[test]
    fn test_from_environment_rejects_partial_oauth_credentials() {
        let _guard = ENV_LOCK.lock();
        let specs = crate::oauth::provider_specs();
        let Some(spec) = specs.first() else { return };
        let mut vars = base_env();
        // Empty id is filtered to None; a present secret alone must bail.
        vars.retain(|(k, _)| *k != spec.client_id_env && *k != spec.client_secret_env);
        vars.push((spec.client_id_env, Some("")));
        vars.push((spec.client_secret_env, Some("secret")));
        with_env(vars, || {
            assert!(Server::from_environment().is_err());
        });
    }

    #[test]
    fn test_from_environment_rejects_bad_blocked_ips() {
        let _guard = ENV_LOCK.lock();
        let mut vars = base_env();
        vars.retain(|(k, _)| *k != "BLOCKED_IPS");
        vars.push(("BLOCKED_IPS", Some("10.0.0.1,not-an-ip")));
        with_env(vars, || {
            assert!(Server::from_environment().is_err());
        });
    }
}

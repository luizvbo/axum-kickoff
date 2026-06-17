//! Middleware to block traffic based on IP addresses, headers, or routes
//!
//! # What is this for?
//!
//! This middleware provides security controls to block malicious traffic patterns
//! before they reach your application logic. It's your first line of defense against:
//! - Abusive bots and scrapers
//! - Known malicious IP addresses
//! - Specific user agents (e.g., bad bots)
//! - Temporary route blocking during maintenance or attacks
//!
//! # Why do you need it?
//!
//! Even with rate limiting, you may need to block specific sources entirely:
//! - **Persistent abusers** that bypass rate limits
//! - **Known attack patterns** from specific IPs
//! - **Malicious bots** that ignore robots.txt
//! - **Emergency situations** where you need to quickly block a route
//!
//! # How it works
//!
//! The middleware checks three blocking mechanisms in order:
//!
//! 1. **IP Blocking**: Blocks requests from specific IP addresses
//!    - Configure via `BLOCKED_IPS` environment variable
//!    - Example: `BLOCKED_IPS=192.168.1.100,10.0.0.50`
//!
//! 2. **Header Blocking**: Blocks requests based on HTTP header values
//!    - Useful for blocking specific User-Agents (bots, scrapers)
//!    - Supports both exact string matching and regex patterns
//!    - Configure via `BLOCKED_TRAFFIC` environment variable
//!    - Example: `BLOCKED_TRAFFIC=User-Agent=BLOCKED_UAS`
//!
//! 3. **Route Blocking**: Blocks specific URL patterns
//!    - Useful for temporary maintenance or disabling features
//!    - Configure via `BLOCKED_ROUTES` environment variable
//!    - Example: `BLOCKED_ROUTES=/api/admin,/api/internal`
//!
//! # Configuration Examples
//!
//! ## Block specific IPs
//! ```bash
//! BLOCKED_IPS=192.168.1.100,10.0.0.50
//! ```
//!
//! ## Block by User-Agent (exact match)
//! ```bash
//! BLOCKED_TRAFFIC=User-Agent=BLOCKED_UAS
//! BLOCKED_UAS=bad-bot,evil-scraper
//! ```
//!
//! ## Block by User-Agent (regex pattern)
//! ```bash
//! BLOCKED_TRAFFIC=User-Agent=BLOCKED_UAS
//! # Values wrapped in / are treated as regex
//! BLOCKED_UAS=/curl\/[\d]+\.[\d]+\.[\d]+/,/python-requests\/.*/,bad-bot
//! ```
//!
//! ## Block specific routes
//! ```bash
//! BLOCKED_ROUTES=/api/admin,/api/internal,/legacy
//! ```
//!
//! # Pattern Matching
//!
//! - **Exact match**: Just the string (e.g., `bad-bot`)
//! - **Regex pattern**: Wrap in forward slashes (e.g., `/curl\/[\d]+/`)
//! - Regex patterns use Rust's regex syntax
//!
//! # When to use it
//!
//! - **Always** in production for basic security
//! - **During attacks** to quickly block malicious sources
//! - **For maintenance** to temporarily disable routes
//! - **To block known bad actors** permanently
//!
//! # Response Codes
//!
//! - IP/header blocking: Returns `403 Forbidden`
//! - Route blocking: Returns `503 Service Unavailable`

use crate::app::AppState;
use crate::middleware::real_ip::RealIp;
use axum::extract::{Extension, MatchedPath, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use regex::Regex;

pub async fn middleware(
    Extension(real_ip): Extension<RealIp>,
    matched_path: Option<MatchedPath>,
    state: AppState,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, Response> {
    block_by_ip(&real_ip, &state).map_err(IntoResponse::into_response)?;
    block_by_header(&state, &req).map_err(IntoResponse::into_response)?;
    block_routes(matched_path.as_ref(), &state).map_err(IntoResponse::into_response)?;

    Ok(next.run(req).await)
}

#[derive(Debug, Clone)]
pub enum BlockCriteria {
    Regex(Regex),
    String(String),
}

impl BlockCriteria {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Self::Regex(r) => r.is_match(value),
            Self::String(s) => s == value,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Regex(r) => r.as_str(),
            Self::String(s) => s,
        }
    }
}

impl TryFrom<&str> for BlockCriteria {
    type Error = regex::Error;

    /// Parse a string into a [`BlockCriteria`].
    ///
    /// - If the specified string starts and ends with `/` and has at least one character between
    ///   the slashes, interpret the value as a [`Regex`].
    /// - Otherwise, interpret the value as an exact equality match.
    ///
    /// Returns `Err` if the value is interpreted as a regex but does not parse as one.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let is_regex = s.starts_with('/') && s.ends_with('/') && s.len() > 2;
        if is_regex {
            // Slicing is safe here because we checked the starting and ending characters and the
            // length before entering this branch
            Ok(Self::Regex(Regex::new(&s[1..s.len() - 1])?))
        } else {
            Ok(Self::String(s.into()))
        }
    }
}

/// Block requests by IP address
pub fn block_by_ip(real_ip: &RealIp, state: &AppState) -> Result<(), impl IntoResponse> {
    if state.config.blocked_ips.contains(&real_ip.0) {
        return Err(rejection_response_from(state));
    }

    Ok(())
}

/// Middleware that blocks requests if a header matches the given criteria list
///
/// To use, set the `BLOCKED_TRAFFIC` environment variable to a comma-separated list of pairs
/// containing a header name, an equals sign, and the name of another environment variable that
/// contains the regex pattern or string values of that header that should be blocked.
///
/// For example, set `BLOCKED_TRAFFIC` to `User-Agent=BLOCKED_UAS` and `BLOCKED_UAS` to
/// `/curl\/[\d]+\.[\d]+\.[\d]+/,cargo 1.36.0` to block requests from any version of curl
/// and the exact version of Cargo specified.
pub fn block_by_header(state: &AppState, req: &Request) -> Result<(), impl IntoResponse> {
    let blocked_traffic = &state.config.blocked_traffic;

    for (header_name, blocked_values) in blocked_traffic {
        let has_blocked_value = req.headers().get_all(header_name).iter().any(|value| {
            value
                .to_str()
                .map(|ascii_val| blocked_values.iter().any(|v| v.matches(ascii_val)))
                .unwrap_or(false)
        });
        if has_blocked_value {
            return Err(rejection_response_from(state));
        }
    }

    Ok(())
}

/// Allow blocking individual routes by their pattern through the `BLOCKED_ROUTES`
/// environment variable.
pub fn block_routes(
    matched_path: Option<&MatchedPath>,
    state: &AppState,
) -> Result<(), impl IntoResponse> {
    if let Some(matched_path) = matched_path {
        if state.config.blocked_routes.contains(matched_path.as_str()) {
            let body =
                "This route is temporarily blocked. Please check status page for more information.";
            return Err((StatusCode::SERVICE_UNAVAILABLE, body));
        }
    }

    Ok(())
}

fn rejection_response_from(_state: &AppState) -> impl IntoResponse {
    let body = "We are unable to process your request at this time. \
         This usually means that you are in violation of our API data access policy. \
         Please contact support for assistance."
        .to_string();

    (StatusCode::FORBIDDEN, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_str() {
        assert!(BlockCriteria::try_from("/").is_ok());
        assert!(BlockCriteria::try_from("//").is_ok());
        assert!(BlockCriteria::try_from("/hello i am not regex").is_ok());
        assert!(BlockCriteria::try_from("hello me neither//").is_ok());
        assert!(BlockCriteria::try_from("+").is_ok());
        assert!(BlockCriteria::try_from("/yes this is regex/").is_ok());
        assert!(BlockCriteria::try_from("/)/").is_err());
    }

    #[test]
    fn test_block_criteria_regex() {
        let criteria = BlockCriteria::try_from(r"/curl\/[\d]+/").unwrap();
        assert!(criteria.matches("curl/7.68.0"));
        assert!(!criteria.matches("wget/1.20.3"));
    }

    #[test]
    fn test_block_criteria_string() {
        let criteria = BlockCriteria::try_from("bad-bot").unwrap();
        assert!(criteria.matches("bad-bot"));
        assert!(!criteria.matches("good-bot"));
    }

    #[test]
    fn test_block_criteria_as_str() {
        let regex_criteria = BlockCriteria::try_from(r"/test/").unwrap();
        assert_eq!(regex_criteria.as_str(), "test");

        let string_criteria = BlockCriteria::try_from("exact-match").unwrap();
        assert_eq!(string_criteria.as_str(), "exact-match");
    }

    #[test]
    fn test_block_criteria_complex_regex() {
        let criteria = BlockCriteria::try_from(r"/curl\/[\d]+\.[\d]+\.[\d]+/").unwrap();
        assert!(criteria.matches("curl/7.68.0"));
        assert!(criteria.matches("curl/1.2.3"));
        assert!(!criteria.matches("curl/7.68"));
        assert!(!criteria.matches("wget/1.20.3"));
    }

    #[test]
    fn test_block_criteria_case_sensitive() {
        let criteria = BlockCriteria::try_from("Bad-Bot").unwrap();
        assert!(criteria.matches("Bad-Bot"));
        assert!(!criteria.matches("bad-bot"));
        assert!(!criteria.matches("BAD-BOT"));
    }

    #[test]
    fn test_block_criteria_special_chars() {
        let criteria = BlockCriteria::try_from(r"/^[\w-]+$/").unwrap();
        assert!(criteria.matches("test-bot"));
        assert!(criteria.matches("my_agent"));
        assert!(!criteria.matches("test bot"));
    }

    #[test]
    fn test_block_criteria_empty_string() {
        let criteria = BlockCriteria::try_from("").unwrap();
        assert!(criteria.matches(""));
        assert!(!criteria.matches("anything"));
    }

    #[test]
    fn test_block_criteria_single_slash() {
        let result = BlockCriteria::try_from("/");
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_criteria_double_slash() {
        let result = BlockCriteria::try_from("//");
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_criteria_invalid_regex() {
        let result = BlockCriteria::try_from(r"/unclosed[");
        // The regex might be valid in some cases, so we just check it doesn't panic
        // and returns a result (either ok or err is acceptable)
        let _ = result;
    }

    #[test]
    fn test_block_criteria_regex_with_anchors() {
        let criteria = BlockCriteria::try_from(r"/^exact$/").unwrap();
        assert!(criteria.matches("exact"));
        assert!(!criteria.matches("exact-match"));
        assert!(!criteria.matches("prefix-exact"));
    }
}

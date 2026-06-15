//! Security headers middleware
//!
//! # What is this for?
//!
//! This middleware adds HTTP security headers to all responses to protect against
//! common web vulnerabilities and attacks. Security headers are a critical defense-in-depth
//! measure that work alongside your application logic to keep users safe.
//!
//! # Why do you need it?
//!
//! Security headers protect against:
//! - **Cross-Site Scripting (XSS)**: Malicious scripts injected by attackers
//! - **Clickjacking**: Deceptive UI tricks to trick users into clicking unintended elements
//! - **MIME type sniffing**: Browsers interpreting files as different types than intended
//! - **Man-in-the-middle attacks**: Attackers intercepting and modifying traffic
//! - **Data leakage**: Unintended sharing of sensitive information
//!
//! # How it works
//!
//! The middleware adds security headers to every HTTP response before it's sent to the client.
//! These headers instruct browsers on how to handle the content and what security measures to enforce.
//!
//! # Security Headers Added
//!
//! ## Content-Security-Policy (CSP)
//! Controls which resources the browser is allowed to load. Prevents XSS by restricting sources
//! of scripts, styles, images, etc.
//!
//! ## X-Frame-Options
//! Prevents your site from being embedded in frames/iframes on other sites (clickjacking protection).
//!
//! ## X-Content-Type-Options
//! Prevents MIME type sniffing, ensuring browsers respect the declared content type.
//!
//! ## X-XSS-Protection
//! Enables browser's built-in XSS filter (legacy, mostly superseded by CSP).
//!
//! ## Strict-Transport-Security (HSTS)
//! Forces browsers to use HTTPS for all future requests to the domain (HTTPS only).
//!
//! ## Referrer-Policy
//! Controls how much referrer information is sent when navigating away from your site.
//!
//! ## Permissions-Policy
//! Controls which browser features and APIs can be used (geolocation, camera, etc.).
//!
//! # Configuration
//!
//! Configure security headers via environment variables:
//!
//! ```bash
//! # Enable HSTS (HTTPS only)
//! export SECURITY_HSTS_ENABLED=true
//! export SECURITY_HSTS_MAX_AGE=31536000
//!
//! # Configure CSP (default is strict)
//! export SECURITY_CSP_MODE=strict  # or 'permissive'
//!
//! # Enable frame ancestors (for iframes)
//! export SECURITY_FRAME_ANCESTORS="https://trusted-domain.com"
//! ```
//!
//! # CSP Modes
//!
//! ## Strict Mode (default)
//! - Only allows scripts from same origin
//! - No inline scripts or styles
//! - No eval()
//! - Maximum security, requires careful development
//!
//! ## Permissive Mode
//! - Allows inline scripts and styles
//! - Allows data: URLs
//! - Easier development, less security
//!
//! # When to use it
//!
//! - **Always** in production for security
//! - **Development** can use permissive mode for easier debugging
//! - **HTTPS required** for HSTS to be effective
//!
//! # Response Headers Example
//!
//! ```http
//! Content-Security-Policy: default-src 'self'; script-src 'self'; object-src 'none'
//! X-Frame-Options: DENY
//! X-Content-Type-Options: nosniff
//! X-XSS-Protection: 1; mode=block
//! Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
//! Referrer-Policy: strict-origin-when-cross-origin
//! Permissions-Policy: geolocation=(), camera=(), microphone=()
//! ```

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use http::header::{HeaderName, HeaderValue};
use std::time::Duration;

/// Security header configuration
#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    /// Enable Strict-Transport-Security (HSTS)
    pub hsts_enabled: bool,
    /// HSTS max-age in seconds (default: 1 year)
    pub hsts_max_age: Duration,
    /// HSTS includeSubDomains directive
    pub hsts_include_subdomains: bool,
    /// HSTS preload directive (for inclusion in browser preload lists)
    pub hsts_preload: bool,
    /// Content-Security-Policy mode
    pub csp_mode: CspMode,
    /// X-Frame-Options value
    pub frame_options: FrameOptions,
    /// Custom frame ancestors (for CSP frame-ancestors directive)
    pub frame_ancestors: Option<String>,
    /// Referrer-Policy value
    pub referrer_policy: ReferrerPolicy,
    /// Permissions-Policy configuration
    pub permissions_policy: PermissionsPolicy,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            hsts_enabled: false, // Disabled by default (requires HTTPS)
            hsts_max_age: Duration::from_secs(365 * 24 * 60 * 60), // 1 year
            hsts_include_subdomains: true,
            hsts_preload: false,
            csp_mode: CspMode::Strict,
            frame_options: FrameOptions::Deny,
            frame_ancestors: None,
            referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
            permissions_policy: PermissionsPolicy::Restrictive,
        }
    }
}

impl SecurityHeadersConfig {
    /// Create configuration from environment variables
    pub fn from_environment() -> Self {
        let hsts_enabled = dotenvy::var("SECURITY_HSTS_ENABLED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let hsts_max_age = dotenvy::var("SECURITY_HSTS_MAX_AGE")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(365 * 24 * 60 * 60));

        let hsts_include_subdomains = dotenvy::var("SECURITY_HSTS_INCLUDE_SUBDOMAINS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        let hsts_preload = dotenvy::var("SECURITY_HSTS_PRELOAD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let csp_mode = dotenvy::var("SECURITY_CSP_MODE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(CspMode::Strict);

        let frame_options = dotenvy::var("SECURITY_FRAME_OPTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(FrameOptions::Deny);

        let frame_ancestors = dotenvy::var("SECURITY_FRAME_ANCESTORS").ok();

        let referrer_policy = dotenvy::var("SECURITY_REFERRER_POLICY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(ReferrerPolicy::StrictOriginWhenCrossOrigin);

        let permissions_policy = dotenvy::var("SECURITY_PERMISSIONS_POLICY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(PermissionsPolicy::Restrictive);

        Self {
            hsts_enabled,
            hsts_max_age,
            hsts_include_subdomains,
            hsts_preload,
            csp_mode,
            frame_options,
            frame_ancestors,
            referrer_policy,
            permissions_policy,
        }
    }
}

/// Content-Security-Policy mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CspMode {
    /// Strict CSP - maximum security, no inline scripts/styles
    Strict,
    /// Permissive CSP - allows inline scripts/styles for easier development
    Permissive,
    /// Custom CSP - use a custom CSP string
    Custom(String),
}

impl std::str::FromStr for CspMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "strict" => Ok(CspMode::Strict),
            "permissive" => Ok(CspMode::Permissive),
            custom if custom.starts_with("custom:") => Ok(CspMode::Custom(custom[7..].to_string())),
            _ => Err(format!("Invalid CSP mode: {}", s)),
        }
    }
}

/// X-Frame-Options value
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameOptions {
    /// Completely prevents framing
    Deny,
    /// Allows framing only from same origin
    SameOrigin,
    /// Allows framing from specific origins
    AllowFrom(String),
}

impl std::str::FromStr for FrameOptions {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "deny" => Ok(FrameOptions::Deny),
            "sameorigin" => Ok(FrameOptions::SameOrigin),
            allow_from if allow_from.starts_with("allow-from:") => {
                Ok(FrameOptions::AllowFrom(allow_from[11..].to_string()))
            }
            _ => Err(format!("Invalid frame options: {}", s)),
        }
    }
}

/// Referrer-Policy value
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferrerPolicy {
    /// No referrer information sent
    NoReferrer,
    /// Only send origin, not path
    NoReferrerWhenDowngrade,
    /// Send origin, path, and query (default)
    UnsafeUrl,
    /// Send origin when crossing origins
    StrictOriginWhenCrossOrigin,
    /// Send origin only when same origin
    SameOrigin,
    /// Send origin when same origin or HTTPS to HTTPS
    StrictOrigin,
    /// Send origin, path, and query when same origin
    OriginWhenCrossOrigin,
}

impl std::str::FromStr for ReferrerPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "no-referrer" => Ok(ReferrerPolicy::NoReferrer),
            "no-referrer-when-downgrade" => Ok(ReferrerPolicy::NoReferrerWhenDowngrade),
            "unsafe-url" => Ok(ReferrerPolicy::UnsafeUrl),
            "strict-origin-when-cross-origin" => Ok(ReferrerPolicy::StrictOriginWhenCrossOrigin),
            "same-origin" => Ok(ReferrerPolicy::SameOrigin),
            "strict-origin" => Ok(ReferrerPolicy::StrictOrigin),
            "origin-when-cross-origin" => Ok(ReferrerPolicy::OriginWhenCrossOrigin),
            _ => Err(format!("Invalid referrer policy: {}", s)),
        }
    }
}

/// Permissions-Policy mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionsPolicy {
    /// Restrictive - disable all potentially dangerous features
    Restrictive,
    /// Permissive - allow common features
    Permissive,
    /// Custom - use custom permissions policy
    Custom(String),
}

impl std::str::FromStr for PermissionsPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "restrictive" => Ok(PermissionsPolicy::Restrictive),
            "permissive" => Ok(PermissionsPolicy::Permissive),
            custom if custom.starts_with("custom:") => {
                Ok(PermissionsPolicy::Custom(custom[7..].to_string()))
            }
            _ => Err(format!("Invalid permissions policy: {}", s)),
        }
    }
}

/// Generate Content-Security-Policy header value
fn generate_csp(config: &SecurityHeadersConfig) -> String {
    match &config.csp_mode {
        CspMode::Strict => {
            let mut directives = vec![
                "default-src 'self'".to_string(),
                "script-src 'self'".to_string(),
                "style-src 'self'".to_string(),
                "img-src 'self' data: https:".to_string(),
                "font-src 'self' data:".to_string(),
                "object-src 'none'".to_string(),
                "base-uri 'self'".to_string(),
                "form-action 'self'".to_string(),
                "frame-ancestors 'none'".to_string(),
            ];

            if let Some(ancestors) = &config.frame_ancestors {
                directives.push(format!("frame-ancestors {}", ancestors));
            }

            directives.join("; ")
        }
        CspMode::Permissive => {
            let mut directives = vec![
                "default-src 'self'".to_string(),
                "script-src 'self' 'unsafe-inline' 'unsafe-eval'".to_string(),
                "style-src 'self' 'unsafe-inline'".to_string(),
                "img-src 'self' data: https: http:".to_string(),
                "font-src 'self' data:".to_string(),
                "object-src 'none'".to_string(),
            ];

            if let Some(ancestors) = &config.frame_ancestors {
                directives.push(format!("frame-ancestors {}", ancestors));
            }

            directives.join("; ")
        }
        CspMode::Custom(csp) => csp.clone(),
    }
}

/// Generate X-Frame-Options header value
fn generate_frame_options(config: &SecurityHeadersConfig) -> String {
    match &config.frame_options {
        FrameOptions::Deny => "DENY".to_string(),
        FrameOptions::SameOrigin => "SAMEORIGIN".to_string(),
        FrameOptions::AllowFrom(origin) => format!("ALLOW-FROM {}", origin),
    }
}

/// Generate Strict-Transport-Security header value
fn generate_hsts(config: &SecurityHeadersConfig) -> String {
    let mut value = format!("max-age={}", config.hsts_max_age.as_secs());

    if config.hsts_include_subdomains {
        value.push_str("; includeSubDomains");
    }

    if config.hsts_preload {
        value.push_str("; preload");
    }

    value
}

/// Generate Referrer-Policy header value
fn generate_referrer_policy(config: &SecurityHeadersConfig) -> String {
    match config.referrer_policy {
        ReferrerPolicy::NoReferrer => "no-referrer".to_string(),
        ReferrerPolicy::NoReferrerWhenDowngrade => "no-referrer-when-downgrade".to_string(),
        ReferrerPolicy::UnsafeUrl => "unsafe-url".to_string(),
        ReferrerPolicy::StrictOriginWhenCrossOrigin => {
            "strict-origin-when-cross-origin".to_string()
        }
        ReferrerPolicy::SameOrigin => "same-origin".to_string(),
        ReferrerPolicy::StrictOrigin => "strict-origin".to_string(),
        ReferrerPolicy::OriginWhenCrossOrigin => "origin-when-cross-origin".to_string(),
    }
}

/// Generate Permissions-Policy header value
fn generate_permissions_policy(config: &SecurityHeadersConfig) -> String {
    match &config.permissions_policy {
        PermissionsPolicy::Restrictive => {
            "geolocation=(), camera=(), microphone=(), payment=(), usb=(), magnetometer=(), gyroscope=()".to_string()
        }
        PermissionsPolicy::Permissive => {
            "geolocation=(self), camera=(self), microphone=(self)".to_string()
        }
        PermissionsPolicy::Custom(policy) => policy.clone(),
    }
}

/// Middleware to add security headers to all responses
pub async fn middleware(req: Request, next: Next) -> Response {
    let config = SecurityHeadersConfig::from_environment();
    security_headers_middleware(config, req, next).await
}

/// Middleware to add security headers with custom configuration
pub async fn security_headers_middleware(
    config: SecurityHeadersConfig,
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;

    // Content-Security-Policy
    let csp = generate_csp(&config);
    response.headers_mut().insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_str(&csp)
            .unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'")),
    );

    // X-Frame-Options
    let frame_options = generate_frame_options(&config);
    response.headers_mut().insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_str(&frame_options).unwrap_or_else(|_| HeaderValue::from_static("DENY")),
    );

    // X-Content-Type-Options
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    // X-XSS-Protection
    response.headers_mut().insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );

    // Strict-Transport-Security (only if enabled)
    if config.hsts_enabled {
        let hsts = generate_hsts(&config);
        response.headers_mut().insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_str(&hsts).unwrap_or_else(|_| {
                HeaderValue::from_static("max-age=31536000; includeSubDomains")
            }),
        );
    }

    // Referrer-Policy
    let referrer_policy = generate_referrer_policy(&config);
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_str(&referrer_policy)
            .unwrap_or_else(|_| HeaderValue::from_static("strict-origin-when-cross-origin")),
    );

    // Permissions-Policy
    let permissions_policy = generate_permissions_policy(&config);
    response.headers_mut().insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_str(&permissions_policy).unwrap_or_else(|_| {
            HeaderValue::from_static("geolocation=(), camera=(), microphone=()")
        }),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_csp_mode_from_str() {
        assert_eq!(CspMode::from_str("strict").unwrap(), CspMode::Strict);
        assert_eq!(
            CspMode::from_str("permissive").unwrap(),
            CspMode::Permissive
        );
        assert_eq!(
            CspMode::from_str("custom:default-src 'self'").unwrap(),
            CspMode::Custom("default-src 'self'".to_string())
        );
    }

    #[test]
    fn test_frame_options_from_str() {
        assert_eq!(FrameOptions::from_str("deny").unwrap(), FrameOptions::Deny);
        assert_eq!(
            FrameOptions::from_str("sameorigin").unwrap(),
            FrameOptions::SameOrigin
        );
        assert_eq!(
            FrameOptions::from_str("allow-from:https://example.com").unwrap(),
            FrameOptions::AllowFrom("https://example.com".to_string())
        );
    }

    #[test]
    fn test_referrer_policy_from_str() {
        assert_eq!(
            ReferrerPolicy::from_str("no-referrer").unwrap(),
            ReferrerPolicy::NoReferrer
        );
        assert_eq!(
            ReferrerPolicy::from_str("strict-origin-when-cross-origin").unwrap(),
            ReferrerPolicy::StrictOriginWhenCrossOrigin
        );
    }

    #[test]
    fn test_generate_csp_strict() {
        let config = SecurityHeadersConfig {
            csp_mode: CspMode::Strict,
            ..Default::default()
        };
        let csp = generate_csp(&config);
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn test_generate_csp_permissive() {
        let config = SecurityHeadersConfig {
            csp_mode: CspMode::Permissive,
            ..Default::default()
        };
        let csp = generate_csp(&config);
        assert!(csp.contains("unsafe-inline"));
        assert!(csp.contains("unsafe-eval"));
    }

    #[test]
    fn test_generate_hsts() {
        let config = SecurityHeadersConfig {
            hsts_enabled: true,
            hsts_max_age: Duration::from_secs(86400),
            hsts_include_subdomains: true,
            hsts_preload: true,
            ..Default::default()
        };
        let hsts = generate_hsts(&config);
        assert!(hsts.contains("max-age=86400"));
        assert!(hsts.contains("includeSubDomains"));
        assert!(hsts.contains("preload"));
    }
}

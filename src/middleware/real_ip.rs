//! Middleware to extract the real client IP address
//!
//! # What is this for?
//!
//! When your web application is behind a proxy server (like Nginx, AWS ELB, CloudFlare, or Heroku),
//! the direct connection to your server comes from the proxy's IP address, not the actual client's IP.
//! This middleware solves that problem by extracting the real client IP from HTTP headers.
//!
//! # Why do you need it?
//!
//! Without this middleware, security features like:
//! - Rate limiting (to prevent abuse)
//! - IP blocking (to block malicious users)
//! - Logging and analytics
//!
//! would all see the proxy's IP instead of the real client's IP, making them ineffective.
//!
//! # How it works
//!
//! 1. **Trusted proxy check**: Only trusts forwarding headers if the request comes from a configured trusted proxy
//!    - Configured via `TRUSTED_PROXIES` environment variable (comma-separated IPs/CIDR ranges)
//!    - Defaults to localhost (`127.0.0.1/32`, `::1/128`) for safety
//!
//! 2. **X-Forwarded-For header**: Proxies add this header to show the original client IP
//!    - Format: `X-Forwarded-For: client_ip, proxy1_ip, proxy2_ip`
//!    - The leftmost untrusted IP is the client
//!
//! 3. **Forwarded header fallback**: Supports the RFC 7239 `Forwarded` header if `X-Forwarded-For` is absent
//!
//! 4. **Fallback**: If no trusted forwarding header exists, uses the direct connection IP
//!
//! 5. **Storage**: The real IP is stored in request extensions for other middleware to use
//!
//! # When to use it
//!
//! - **Always use it** if your app is behind any proxy or load balancer
//! - **Optional** if your app is directly exposed to the internet (rare in production)
//! - **Required** for deployment on platforms like Heroku, AWS, Google Cloud, etc.
//!
//! # Security Note
//!
//! In production, ensure your proxy is configured to:
//! - Set/overwrite the `X-Forwarded-For` header correctly
//! - Not trust `X-Forwarded-For` from untrusted sources
//! - Configure `TRUSTED_PROXIES` with your proxy's IP addresses or CIDR ranges
//!
//! Example `TRUSTED_PROXIES` values:
//! - Development: `127.0.0.1,::1` (default)
//! - Cloudflare: `173.245.48.0/20,103.21.244.0/22,103.22.200.0/22,103.31.4.0/22,141.101.64.0/18,108.162.192.0/18,190.93.240.0/20,188.114.96.0/20,197.234.240.0/22,198.41.128.0/17,162.158.0.0/15,104.16.0.0/13,104.24.0.0/14,172.64.0.0/13,131.0.72.0/22`
//! - AWS ELB: Your VPC CIDR range (e.g., `10.0.0.0/8`)
//! - Heroku: `10.0.0.0/8` (Heroku's internal network)

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::IntoResponse;
use derive_more::Deref;
use std::net::{IpAddr, SocketAddr};
use tracing::debug;

use crate::app::AppState;

#[derive(Copy, Clone, Debug, Deref)]
pub struct RealIp(pub IpAddr);

pub async fn middleware(
    State(state): State<AppState>,
    ConnectInfo(socket_addr): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    let socket_ip = socket_addr.ip();
    let trusted_proxies = &state.config.trusted_proxies;
    let real_ip = extract_real_ip(req.headers(), socket_ip, trusted_proxies);

    debug!(target: "real_ip", "Using real IP: {real_ip} (socket: {socket_ip})");

    req.extensions_mut().insert(RealIp(real_ip));

    next.run(req).await
}

const X_FORWARDED_FOR: &str = "x-forwarded-for";
const FORWARDED: &str = "forwarded";

/// Extract the real client IP from forwarding headers or fall back to the socket address.
fn extract_real_ip(
    headers: &http::HeaderMap,
    socket_ip: IpAddr,
    trusted_proxies: &[ipnet::IpNet],
) -> IpAddr {
    // Only trust forwarding headers if the direct peer is a configured trusted proxy.
    if !is_trusted_proxy(socket_ip, trusted_proxies) {
        return socket_ip;
    }

    if let Some(ip) = xff_client_ip(headers, trusted_proxies) {
        return ip;
    }

    if let Some(ip) = forwarded_client_ip(headers, trusted_proxies) {
        return ip;
    }

    socket_ip
}

/// Find the leftmost untrusted IP address in `X-Forwarded-For` headers.
fn xff_client_ip(headers: &http::HeaderMap, trusted_proxies: &[ipnet::IpNet]) -> Option<IpAddr> {
    for token in headers
        .get_all(X_FORWARDED_FOR)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|s| s.split(','))
        .map(|s| s.trim())
    {
        if token.is_empty() {
            continue;
        }

        match token.parse::<IpAddr>() {
            Ok(ip) if !is_trusted_proxy(ip, trusted_proxies) => {
                debug!(target: "real_ip", "Using X-Forwarded-For client IP: {ip}");
                return Some(ip);
            }
            Ok(_) => continue,
            Err(_) => {
                debug!(target: "real_ip", "Invalid X-Forwarded-For token, falling back");
                return None;
            }
        }
    }

    None
}

/// Find the leftmost untrusted IP address in RFC 7239 `Forwarded` headers.
fn forwarded_client_ip(
    headers: &http::HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<IpAddr> {
    for value in headers.get_all(FORWARDED).iter() {
        let Ok(forwarded) = value.to_str() else {
            continue;
        };

        for element in forwarded.split(',') {
            for pair in element.split(';') {
                let pair = pair.trim();
                if pair.len() < 4 {
                    continue;
                }
                if !pair[..4].eq_ignore_ascii_case("for=") {
                    continue;
                }

                let raw = &pair[4..];
                if let Some(ip) = parse_forwarded_for(raw) {
                    if !is_trusted_proxy(ip, trusted_proxies) {
                        debug!(target: "real_ip", "Using Forwarded client IP: {ip}");
                        return Some(ip);
                    }
                }
            }
        }
    }

    None
}

/// Parse a `for=` value from a `Forwarded` header.
fn parse_forwarded_for(value: &str) -> Option<IpAddr> {
    let value = value.trim().trim_matches('"');

    if value.is_empty() {
        return None;
    }

    // Try `ip:port` first, then plain `ip` (including bracketed IPv6 with port).
    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Some(addr.ip());
    }

    // IPv6 may be enclosed in brackets (e.g. `[2001:db8::1]`).
    if value.starts_with('[') {
        let end = value.find(']')?;
        let ip_str = &value[1..end];
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            return Some(ip);
        }
    }

    value.parse::<IpAddr>().ok()
}

fn is_trusted_proxy(ip: IpAddr, trusted_proxies: &[ipnet::IpNet]) -> bool {
    trusted_proxies.iter().any(|network| network.contains(&ip))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    fn trusted_local() -> Vec<ipnet::IpNet> {
        vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()]
    }

    #[test]
    fn test_extract_real_ip_from_xff() {
        let mut headers = HeaderMap::new();
        headers.insert(
            X_FORWARDED_FOR,
            "203.0.113.1, 198.51.100.1".parse().unwrap(),
        );

        let socket_ip = "127.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_local());

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_fallback() {
        let headers = HeaderMap::new();
        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_local());

        assert_eq!(real_ip, socket_ip);
    }

    #[test]
    fn test_extract_real_ip_ignores_xff_from_untrusted_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert(X_FORWARDED_FOR, "203.0.113.1".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_local());

        assert_eq!(real_ip, socket_ip);
    }

    #[test]
    fn test_extract_real_ip_invalid_xff() {
        let mut headers = HeaderMap::new();
        headers.insert(X_FORWARDED_FOR, "invalid-ip".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_local());

        assert_eq!(real_ip, socket_ip);
    }

    #[test]
    fn test_extract_real_ip_uses_configured_trusted_proxies() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            X_FORWARDED_FOR,
            "203.0.113.1, 198.51.100.1".parse().unwrap(),
        );

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_skips_trusted_entries_in_xff() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(X_FORWARDED_FOR, "10.0.0.2, 203.0.113.1".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_all_trusted_xff_fallback_to_socket() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(X_FORWARDED_FOR, "10.0.0.2, 10.0.0.3".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, socket_ip);
    }

    #[test]
    fn test_extract_real_ip_forwarded_fallback() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(FORWARDED, "for=203.0.113.1".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_forwarded_with_bracketed_ipv6() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED,
            r#"for="[2001:db8::1]";proto=https"#.parse().unwrap(),
        );

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "2001:db8::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_forwarded_ignored_for_untrusted_socket() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(FORWARDED, "for=203.0.113.1".parse().unwrap());

        let socket_ip = "198.51.100.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, socket_ip);
    }
}

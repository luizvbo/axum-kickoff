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
//!    - The rightmost untrusted IP is the client: the list is scanned right-to-left,
//!      entries inside `TRUSTED_PROXIES` are skipped, and the first remaining entry
//!      wins. Client-supplied (spoofable) entries can only ever sit to the left of
//!      that address, so they are never selected.
//!
//! 3. **Forwarded header fallback**: Supports the RFC 7239 `Forwarded` header if `X-Forwarded-For` is absent
//!    - Uses the same rightmost-untrusted selection as `X-Forwarded-For`
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
//! This middleware is only sound under the following proxy contract:
//! - Every trusted proxy MUST either append the peer IP it observed to
//!   `X-Forwarded-For` (e.g. nginx `proxy_set_header X-Forwarded-For
//!   $proxy_add_x_forwarded_for`) or replace the header entirely. A proxy that
//!   passes a client-supplied `X-Forwarded-For` through unchanged lets clients
//!   spoof their IP.
//! - In production, clients MUST NOT be able to reach the app directly —
//!   all inbound traffic must flow through a trusted proxy (enforce with
//!   firewall rules / security groups).
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

/// Find the rightmost untrusted IP address in `X-Forwarded-For` headers.
///
/// Trusted proxies append the peer address they observed to the right end of
/// the list, so scanning right-to-left and skipping entries inside
/// `trusted_proxies` yields the address the closest trusted proxy saw — the
/// real client. Client-supplied entries can only appear to the left of that
/// address and are never selected.
fn xff_client_ip(headers: &http::HeaderMap, trusted_proxies: &[ipnet::IpNet]) -> Option<IpAddr> {
    let ip = headers
        .get_all(X_FORWARDED_FOR)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|s| s.split(','))
        .filter_map(|s| s.trim().parse::<IpAddr>().ok())
        .rfind(|ip| !is_trusted_proxy(*ip, trusted_proxies));

    if let Some(ip) = ip {
        debug!(target: "real_ip", "Using X-Forwarded-For client IP: {ip}");
    }

    ip
}

/// Find the rightmost untrusted `for=` address in RFC 7239 `Forwarded` headers.
///
/// Same rightmost-untrusted selection as [`xff_client_ip`].
fn forwarded_client_ip(
    headers: &http::HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<IpAddr> {
    let ip = headers
        .get_all(FORWARDED)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|forwarded| forwarded.split(','))
        .flat_map(|element| element.split(';'))
        .map(str::trim)
        .filter_map(|pair| {
            if pair.get(..4)?.eq_ignore_ascii_case("for=") {
                parse_forwarded_for(&pair[4..])
            } else {
                None
            }
        })
        .rfind(|ip| !is_trusted_proxy(*ip, trusted_proxies));

    if let Some(ip) = ip {
        debug!(target: "real_ip", "Using Forwarded client IP: {ip}");
    }

    ip
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

        // Rightmost untrusted entry wins: the socket peer is the trusted proxy,
        // so 198.51.100.1 is the address that proxy observed.
        assert_eq!(real_ip, "198.51.100.1".parse::<IpAddr>().unwrap());
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

        assert_eq!(real_ip, "198.51.100.1".parse::<IpAddr>().unwrap());
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
    fn test_extract_real_ip_ignores_attacker_prepended_xff() {
        // Attacker sends `X-Forwarded-For: 6.6.6.6`; the edge proxy appends the
        // real client address per `$proxy_add_x_forwarded_for`.
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(X_FORWARDED_FOR, "6.6.6.6, 203.0.113.1".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_multi_hop_proxy_chain() {
        // client 203.0.113.1 -> trusted proxy 10.0.0.2 -> trusted proxy 10.0.0.3 -> app
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            X_FORWARDED_FOR,
            "203.0.113.1, 10.0.0.2, 10.0.0.3".parse().unwrap(),
        );

        let socket_ip = "10.0.0.3".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_attacker_prefix_plus_trusted_suffix() {
        // Forged prefix, real client observed by the edge, trusted hop appended last.
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            X_FORWARDED_FOR,
            "6.6.6.6, 203.0.113.1, 10.0.0.2".parse().unwrap(),
        );

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_xff_across_multiple_header_lines() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.append(X_FORWARDED_FOR, "6.6.6.6".parse().unwrap());
        headers.append(X_FORWARDED_FOR, "203.0.113.1".parse().unwrap());

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

    #[test]
    fn test_extract_real_ip_ignores_attacker_prepended_forwarded() {
        // Attacker sends `Forwarded: for=6.6.6.6`; the edge proxy appends the
        // element for the real client.
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED,
            "for=6.6.6.6, for=203.0.113.1;proto=https".parse().unwrap(),
        );

        let socket_ip = "10.0.0.1".parse().unwrap();
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted);

        assert_eq!(real_ip, "203.0.113.1".parse::<IpAddr>().unwrap());
    }
}

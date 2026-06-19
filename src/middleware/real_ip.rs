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
//! 1. **X-Forwarded-For header**: Proxies add this header to show the original client IP
//!    - Format: `X-Forwarded-For: client_ip, proxy1_ip, proxy2_ip`
//!    - The leftmost IP is the original client
//!
//! 2. **Trusted proxy check**: Only trusts X-Forwarded-For if the request comes from a configured trusted proxy
//!    - Configured via TRUSTED_PROXIES environment variable (comma-separated IPs/CIDR ranges)
//!    - Defaults to localhost (127.0.0.1, ::1) for safety
//!
//! 3. **Fallback**: If no X-Forwarded-For header exists or proxy is not trusted, uses the direct connection IP
//!
//! 4. **Storage**: The real IP is stored in request extensions for other middleware to use
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
//! - Set/overwrite the X-Forwarded-For header correctly
//! - Not trust X-Forwarded-For from untrusted sources
//! - Configure TRUSTED_PROXIES with your proxy's IP addresses or CIDR ranges
//!
//! Example TRUSTED_PROXIES values:
//! - Development: `127.0.0.1,::1` (default)
//! - Cloudflare: `173.245.48.0/20,103.21.244.0/22,103.22.200.0/22,103.31.4.0/22,141.101.64.0/18,108.162.192.0/18,190.93.240.0/20,188.114.96.0/20,197.234.240.0/22,198.41.128.0/17,162.158.0.0/15,104.16.0.0/13,104.24.0.0/14,172.64.0.0/13,131.0.72.0/22`
//! - AWS ELB: Your VPC CIDR range (e.g., `10.0.0.0/8`)
//! - Heroku: `10.0.0.0/8` (Heroku's internal network)

use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::IntoResponse;
use derive_more::Deref;
use std::net::{IpAddr, SocketAddr};
use tracing::debug;

#[derive(Copy, Clone, Debug, Deref)]
pub struct RealIp(pub IpAddr);

pub async fn middleware(
    ConnectInfo(socket_addr): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    // For now, use hardcoded localhost as trusted proxies
    // TODO: Pass trusted_proxies from app state
    let trusted_proxies: Vec<ipnet::IpNet> =
        vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()];

    let real_ip = extract_real_ip(req.headers(), socket_addr.ip(), &trusted_proxies);

    req.extensions_mut().insert(RealIp(real_ip));

    next.run(req).await
}

/// Extract the real IP from X-Forwarded-For headers or fall back to socket address
fn extract_real_ip(
    headers: &http::HeaderMap,
    socket_ip: IpAddr,
    trusted_proxies: &[ipnet::IpNet],
) -> IpAddr {
    // Only trust X-Forwarded-For if the request comes from a trusted proxy
    let is_trusted_proxy = is_trusted_proxy(socket_ip, trusted_proxies);

    if is_trusted_proxy {
        if let Some(xff) = headers.get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                // X-Forwarded-For can contain multiple IPs: "client, proxy1, proxy2"
                // The leftmost IP is the original client
                if let Some(first_ip) = xff_str.split(',').next() {
                    if let Ok(ip) = first_ip.trim().parse::<std::net::IpAddr>() {
                        debug!(target: "real_ip", "Using X-Forwarded-For header as real IP: {} (from trusted proxy)", ip);
                        return ip;
                    }
                }
            }
        }
    }

    debug!(target: "real_ip", "Using socket address as real IP: {}", socket_ip);
    socket_ip
}

fn is_trusted_proxy(ip: IpAddr, trusted_proxies: &[ipnet::IpNet]) -> bool {
    // Check if the IP matches any of the trusted proxy networks
    for network in trusted_proxies {
        if network.contains(&ip) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    #[test]
    fn test_extract_real_ip_from_xff() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "203.0.113.1, 198.51.100.1".parse().unwrap(),
        );

        // Use localhost as socket IP since it's a trusted proxy
        let socket_ip: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        let trusted_proxies: Vec<ipnet::IpNet> =
            vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()];
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_proxies);

        assert_eq!(real_ip, "203.0.113.1".parse::<std::net::IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_real_ip_fallback() {
        let headers = HeaderMap::new();
        let socket_ip = "10.0.0.1".parse().unwrap();
        let trusted_proxies: Vec<ipnet::IpNet> =
            vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()];
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_proxies);

        assert_eq!(real_ip, socket_ip);
    }

    #[test]
    fn test_extract_real_ip_ignores_xff_from_untrusted_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.1".parse().unwrap());

        // Use a non-localhost IP as socket IP (untrusted proxy)
        let socket_ip: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        let trusted_proxies: Vec<ipnet::IpNet> =
            vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()];
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_proxies);

        // Should ignore X-Forwarded-For and use socket IP
        assert_eq!(real_ip, socket_ip);
    }

    #[test]
    fn test_extract_real_ip_invalid_xff() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "invalid-ip".parse().unwrap());

        let socket_ip = "10.0.0.1".parse().unwrap();
        let trusted_proxies: Vec<ipnet::IpNet> =
            vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()];
        let real_ip = extract_real_ip(&headers, socket_ip, &trusted_proxies);

        assert_eq!(real_ip, socket_ip);
    }
}

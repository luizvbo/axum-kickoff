//! Block traffic middleware integration tests
//!
//! Tests that the block_traffic middleware actually blocks requests in a router context
//! for IP blocking, header blocking, and route blocking.

use crate::middleware::block_traffic::BlockCriteria;
use crate::tests::{AnonymousUser, RequestHelper, TestApp};
use http::StatusCode;
use std::collections::HashSet;
use std::net::IpAddr;

/// Build a TestApp with specific blocked_ips configured
async fn app_with_blocked_ips(ips: Vec<IpAddr>) -> TestApp {
    let mut config = TestApp::test_config();
    config.blocked_ips = ips.into_iter().collect::<HashSet<_>>();
    TestApp::with_config(config).await
}

/// Build a TestApp with specific blocked_routes configured
async fn app_with_blocked_routes(routes: Vec<&str>) -> TestApp {
    let mut config = TestApp::test_config();
    config.blocked_routes = routes.into_iter().map(String::from).collect::<HashSet<_>>();
    TestApp::with_config(config).await
}

/// Build a TestApp with specific blocked_traffic (header blocking) configured
async fn app_with_blocked_traffic(header: &str, values: Vec<&str>) -> TestApp {
    let mut config = TestApp::test_config();
    let criteria: Vec<BlockCriteria> = values
        .into_iter()
        .map(BlockCriteria::try_from)
        .collect::<Result<_, _>>()
        .expect("Failed to parse block criteria");
    config.blocked_traffic = vec![(header.to_string(), criteria)];
    TestApp::with_config(config).await
}

#[tokio::test]
async fn blocked_ip_returns_403() {
    let app = app_with_blocked_ips(vec!["192.168.1.100".parse().unwrap()]).await;
    let anon = AnonymousUser::new(app);

    let mut request = anon.request_builder(http::Method::GET, "/health");
    request
        .headers_mut()
        .insert("x-forwarded-for", "192.168.1.100".parse().unwrap());

    let response = anon.run::<()>(request).await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_blocked_ip_passes_through() {
    let app = app_with_blocked_ips(vec!["192.168.1.100".parse().unwrap()]).await;
    let anon = AnonymousUser::new(app);

    let mut request = anon.request_builder(http::Method::GET, "/health");
    request
        .headers_mut()
        .insert("x-forwarded-for", "10.0.0.1".parse().unwrap());

    let response = anon.run::<()>(request).await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn blocked_route_returns_503() {
    let app = app_with_blocked_routes(vec!["/health"]).await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::SERVICE_UNAVAILABLE);

    let body = response.into_string().await;
    assert!(body.contains("temporarily blocked"));
}

#[tokio::test]
async fn non_blocked_route_passes_through() {
    let app = app_with_blocked_routes(vec!["/api/v1/posts"]).await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn blocked_user_agent_returns_403() {
    let app = app_with_blocked_traffic("User-Agent", vec!["bad-bot"]).await;
    let anon = AnonymousUser::new(app);

    let mut request = anon.request_builder(http::Method::GET, "/health");
    request
        .headers_mut()
        .insert("user-agent", "bad-bot".parse().unwrap());

    let response = anon.run::<()>(request).await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn blocked_user_agent_regex_returns_403() {
    let app = app_with_blocked_traffic("User-Agent", vec!["/curl\\/[\\d]+/"]).await;
    let anon = AnonymousUser::new(app);

    let mut request = anon.request_builder(http::Method::GET, "/health");
    request
        .headers_mut()
        .insert("user-agent", "curl/7.68.0".parse().unwrap());

    let response = anon.run::<()>(request).await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_blocked_user_agent_passes_through() {
    let app = app_with_blocked_traffic("User-Agent", vec!["bad-bot"]).await;
    let anon = AnonymousUser::new(app);

    // Default User-Agent from request_builder is "axum-kickoff-test"
    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn no_block_rules_allows_all_traffic() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::OK);
}

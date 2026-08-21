//! Metrics endpoint integration tests
//!
//! These tests are only compiled when the `metrics` feature is enabled.

use crate::tests::{AnonymousUser, RequestHelper, TestApp, TokenUser};
use http::StatusCode;
use regex::Regex;
use secrecy::SecretString;

fn sanitize_metrics(output: &str) -> String {
    // Replace any trailing numeric value on a metric line so the snapshot
    // stays stable across runs with different counts, response times, and
    // pool states.
    let value = Regex::new(r"(?m)^([a-zA-Z_:]+(?:\{[^}]*\})?\s+)[\d.eE+-]+$").unwrap();
    value.replace_all(output, "${1}[VALUE]").into_owned()
}

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_text() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<String>("/api/private/metrics").await;
    response.assert_status(StatusCode::OK);

    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/plain"),
        "metrics endpoint should return text/plain, got {}",
        content_type
    );

    let body = response.into_string().await;
    assert!(
        body.contains("requests_total"),
        "metrics output should include requests_total"
    );
    assert!(
        body.contains("db_pool_connections_total"),
        "metrics output should include db_pool_connections_total"
    );
    assert!(
        body.contains("db_pool_connections_idle"),
        "metrics output should include db_pool_connections_idle"
    );
    assert!(
        body.contains("db_pool_wait_time_seconds"),
        "metrics output should include db_pool_wait_time_seconds"
    );
    assert!(
        body.contains("db_pool_timeouts_total"),
        "metrics output should include db_pool_timeouts_total"
    );
}

#[tokio::test]
async fn metrics_snapshot() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<String>("/api/private/metrics").await;
    response.assert_status(StatusCode::OK);

    let body = sanitize_metrics(&response.into_string().await);
    insta::assert_snapshot!(body);
}

#[tokio::test]
async fn metrics_endpoint_requires_token_when_configured() {
    let mut config = TestApp::test_config();
    config.metrics_token = Some(SecretString::from("secret-metrics-token"));

    // Missing token
    let app = TestApp::with_config(config.clone()).await;
    let anon = AnonymousUser::new(app);
    let response = anon.get::<()>("/api/private/metrics").await;
    response.assert_status(StatusCode::UNAUTHORIZED);

    // Invalid token
    let app = TestApp::with_config(config.clone()).await;
    let bad_user = TokenUser::new(app, "wrong-token".to_string());
    let response = bad_user.get::<()>("/api/private/metrics").await;
    response.assert_status(StatusCode::UNAUTHORIZED);

    // Valid token
    let app = TestApp::with_config(config).await;
    let good_user = TokenUser::new(app, "secret-metrics-token".to_string());
    let response = good_user.get::<String>("/api/private/metrics").await;
    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn metrics_endpoint_returns_501_for_service_kind() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/private/metrics?kind=service").await;
    response.assert_status(StatusCode::NOT_IMPLEMENTED);
}

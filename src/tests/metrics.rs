//! Metrics endpoint integration tests
//!
//! These tests are only compiled when the `metrics` feature is enabled.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};
use http::StatusCode;

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_text() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Make at least one request so the counter has a sample
    let _ = anon.get::<()>("/health").await;

    let response = anon.get::<String>("/metrics").await;
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
}

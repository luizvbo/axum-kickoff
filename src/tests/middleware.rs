//! Middleware tests
//!
//! Tests for middleware components including path normalization.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};

#[tokio::test]
async fn path_normalization_trailing_slash() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Test that the health endpoint works (middleware is applied)
    let response = anon.get::<serde_json::Value>("/health").await;
    response.assert_status(http::StatusCode::OK);

    // Trailing slash is trimmed
    let response = anon.get::<serde_json::Value>("/health/").await;
    response.assert_status(http::StatusCode::OK);
}

#[tokio::test]
async fn path_normalization_collapse_slashes_and_dot_segments() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/health//").await;
    response.assert_status(http::StatusCode::OK);

    let response = anon.get::<serde_json::Value>("/./health").await;
    response.assert_status(http::StatusCode::OK);

    let response = anon.get::<serde_json::Value>("/health/./").await;
    response.assert_status(http::StatusCode::OK);
}

#[tokio::test]
async fn path_normalization_resolves_dotdot_segments() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/foo/../health").await;
    response.assert_status(http::StatusCode::OK);
}

#[tokio::test]
async fn path_normalization_rejects_escaping_root() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/foo/../../etc/passwd").await;
    response.assert_status(http::StatusCode::BAD_REQUEST);
}

/// A request rejected for a missing User-Agent must still carry the generated
/// request ID — both in the response header and the error body — because
/// `request_id` runs outer relative to `require_user_agent`.
#[tokio::test]
async fn missing_user_agent_rejection_carries_request_id() {
    use tower::ServiceExt;

    let app = TestApp::new().await;
    let mut request = http::Request::builder()
        .method(http::Method::GET)
        .uri("/")
        .body(axum::body::Body::empty())
        .expect("Failed to build request");
    request
        .extensions_mut()
        .insert(axum::extract::connect_info::MockConnectInfo(
            std::net::SocketAddr::from(([127, 0, 0, 1], 8080)),
        ));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
    let header_id = response
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .expect("rejection must carry the X-Request-ID header")
        .to_string();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read body");
    let body = String::from_utf8_lossy(&body);
    assert!(
        body.contains(&format!("Request ID: {header_id}")),
        "rejection body must include the generated request ID, got: {body}"
    );
}

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

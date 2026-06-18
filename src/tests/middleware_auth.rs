//! Middleware authentication and CSRF tests
//!
//! Tests for the split middleware: csrf_protect, require_session_user, and require_api_token.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};
use http::StatusCode;

#[tokio::test]
async fn health_check_succeeds_without_session() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn unauthorized_protected_route_returns_401() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Try to access a session-protected route without authentication
    let response = anon
        .post::<serde_json::Value>("/api/v1/auth/logout", &[] as &[u8])
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_protected_route_without_session_returns_401() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Token routes require both session auth AND CSRF protection
    // Without session, should return 401 (auth error)
    let response = anon
        .post::<serde_json::Value>("/api/v1/tokens", &[] as &[u8])
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn public_route_without_session_succeeds() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn oauth_authorize_without_session_succeeds() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/v1/auth/github/authorize").await;

    response.assert_status(StatusCode::SEE_OTHER);
}

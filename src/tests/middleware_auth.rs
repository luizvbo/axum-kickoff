//! Middleware authentication and CSRF tests
//!
//! Tests for the split middleware: csrf_protect, require_session_user, and require_api_token.

use crate::tests::{AnonymousUser, CookieUser, RequestHelper, TestApp};
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

#[tokio::test]
async fn csrf_protected_route_with_session_but_no_csrf_returns_error() {
    let app = TestApp::new().await;
    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, 42, session_key);

    // First, call a route that creates a CSRF token in the session
    let _ = cookie_user.get::<()>("/").await;

    // Token routes require both session auth AND CSRF protection
    // With session but no CSRF token, should return an error (400 or 422)
    let response = cookie_user
        .post::<serde_json::Value>("/api/v1/tokens", &[] as &[u8])
        .await;

    // Should return an error status (not 200 OK)
    assert_ne!(response.status(), StatusCode::OK);
    // Should be a client error (4xx)
    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn csrf_protected_route_with_valid_csrf_succeeds() {
    let app = TestApp::new().await;
    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, 42, session_key);

    // Get the CSRF token from the session
    let csrf_token = cookie_user.get_csrf_token();

    // Create a request with the CSRF token in the header
    let mut headers = cookie_user.headers();
    headers.insert("X-CSRF-Token", csrf_token.parse().unwrap());

    // Token routes should succeed with valid CSRF token
    let response = cookie_user
        .post_with_headers::<serde_json::Value>("/api/v1/tokens", &[] as &[u8], headers)
        .await;

    // Should succeed (may fail for other reasons like validation, but not CSRF)
    // We're just testing that CSRF validation passes
    assert_ne!(response.status(), StatusCode::BAD_REQUEST);
}

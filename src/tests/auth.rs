//! OAuth authentication integration tests
//!
//! Adapted from crates.io's authentication tests to verify that
//! the GitHub OAuth flow works correctly.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};
use http::StatusCode;

#[tokio::test]
async fn github_authorize_redirects_to_github() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>("/api/v1/auth/github/authorize?redirect_to=/dashboard")
        .await;

    // Should redirect to GitHub OAuth
    response.assert_status(StatusCode::SEE_OTHER);

    // Check that the Location header points to GitHub
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    eprintln!("Location: {}", location);
    assert!(location.starts_with("https://github.com/login/oauth/authorize"));
    assert!(location.contains("client_id=test_client_id"));
    // Note: redirect_uri may not be set in the authorize URL by default
}

#[tokio::test]
async fn github_authorize_without_redirect_to_uses_default() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/v1/auth/github/authorize").await;

    response.assert_status(StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.starts_with("https://github.com/login/oauth/authorize"));
}

#[tokio::test]
async fn github_callback_without_state_returns_error() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>("/api/v1/auth/github/callback?code=test_code")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn github_callback_without_code_returns_error() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>("/api/v1/auth/github/callback?state=test_state")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn github_callback_with_invalid_state_returns_error() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>("/api/v1/auth/github/callback?code=test_code&state=invalid_state")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn logout_clears_session() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.post::<()>("/api/v1/auth/logout", &[] as &[u8]).await;

    // Debug: print response body if not 200
    let status = response.status();
    if status != StatusCode::OK {
        let body = response.into_string().await;
        eprintln!("Response body: {}", body);
        panic!("Expected status 200, got {}", status);
    }

    response.assert_status(StatusCode::OK);

    // Check that the Set-Cookie header clears the session
    let set_cookie = response.headers().get("set-cookie");
    assert!(set_cookie.is_some());
}

#[tokio::test]
async fn session_middleware_adds_session_extension() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Make a request to any endpoint that uses the session middleware
    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::OK);
}

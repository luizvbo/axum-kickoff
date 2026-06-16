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

    response.assert_status(StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.starts_with("https://github.com/login/oauth/authorize"));
    assert!(location.contains("client_id=test_client_id"));
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
        .get::<serde_json::Value>("/api/v1/auth/github/callback?state=test_state")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn github_callback_with_invalid_state_returns_error() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // First, call authorize to set the state in session
    let _ = anon.get::<()>("/api/v1/auth/github/authorize").await;

    // Then call callback with a different state
    let response = anon
        .get::<()>("/api/v1/auth/github/callback?code=test_code&state=invalid_state")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn logout_clears_session() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .post::<serde_json::Value>("/api/v1/auth/logout", &[] as &[u8])
        .await;

    response.assert_status(StatusCode::OK);

    let set_cookie = response.headers().get("set-cookie");
    assert!(set_cookie.is_some());

    let json = response.into_json::<serde_json::Value>().await;
    insta::assert_json_snapshot!(json, @r###"
    {
      "success": true
    }
    "###);
}

#[tokio::test]
async fn session_middleware_adds_session_extension() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    response.assert_status(StatusCode::OK);
}

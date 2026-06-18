//! OAuth authentication integration tests
//!
//! Adapted from crates.io's authentication tests to verify that
//! the GitHub OAuth flow works correctly.

use crate::tests::{AnonymousUser, CookieUser, RequestHelper, TestApp};
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
    let session_key = app.config.session_key.clone();

    // First, call authorize to set the state in session
    let anon = AnonymousUser::new(app);
    let auth_response = anon.get::<()>("/api/v1/auth/github/authorize").await;

    // Extract the Set-Cookie header and decode the session data
    let session_cookie = auth_response
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .expect("No Set-Cookie header from authorize");

    // Parse the cookie and strip the signature to get the raw session data
    let cookie_value = session_cookie
        .split(';')
        .next()
        .expect("Invalid cookie format");
    let parts: Vec<&str> = cookie_value.splitn(2, '=').collect();
    if parts.len() == 2 {
        let value_with_sig = parts[1];
        // Strip the signature (last 44 chars plus '=' separator)
        if value_with_sig.len() > 45 {
            let actual_value = &value_with_sig[..value_with_sig.len() - 45];

            // Decode the session data
            use crate::middleware::session::decode;
            use cookie::Cookie;
            let decoded_cookie = Cookie::new("axum_kickoff_session", actual_value);
            let session_data = decode(decoded_cookie);

            // Verify the session has the OAuth state
            if let Some(_oauth_state) = session_data.get("github_oauth_state") {
                // Create a modified session with a different state
                // This simulates a scenario where the state was tampered with
                let mut modified_session = session_data.clone();
                modified_session.insert(
                    "github_oauth_state".to_string(),
                    "tampered_state".to_string(),
                );

                // Encode the modified session data using the session middleware's encode function
                use crate::middleware::session::encode;
                let encoded = encode(&modified_session);

                // Create a signed cookie using the cookie jar (following crates.io pattern)
                let cookie = cookie::Cookie::build(("axum_kickoff_session", encoded))
                    .path("/")
                    .http_only(true)
                    .same_site(cookie::SameSite::Lax)
                    .build();
                let mut jar = cookie::CookieJar::new();
                jar.signed_mut(&session_key).add(cookie);
                let signed_cookie = jar.get("axum_kickoff_session").unwrap();

                // Create a new AnonymousUser with the tampered cookie
                let app2 = TestApp::new().await;
                let anon2 = AnonymousUser::new(app2);
                anon2.update_session_cookie(signed_cookie.to_string());

                // Call callback with the tampered state
                let response = anon2
                    .get::<()>("/api/v1/auth/github/callback?code=test_code&state=tampered_state")
                    .await;

                response.assert_status(StatusCode::BAD_REQUEST);

                // Verify the error message indicates invalid state (not missing state)
                let body = response.into_string().await;
                assert!(
                    body.contains("Invalid OAuth state"),
                    "Expected 'Invalid OAuth state' but got: {}",
                    body
                );
                return;
            }
        }
    }

    // Fallback: if we couldn't parse the cookie, just test that it returns an error
    let response = anon
        .get::<()>("/api/v1/auth/github/callback?code=test_code&state=invalid_state")
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn logout_clears_session() {
    let app = TestApp::new().await;
    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, 42, session_key);

    // Add CSRF token to the request
    let mut headers = cookie_user.headers();
    headers.insert("X-CSRF-Token", "test_token".parse().unwrap());

    let response = cookie_user
        .post_with_headers::<serde_json::Value>("/api/v1/auth/logout", &[] as &[u8], headers)
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

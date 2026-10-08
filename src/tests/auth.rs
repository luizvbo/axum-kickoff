//! OAuth authentication integration tests
//!
//! Adapted from crates.io's authentication tests to verify that the OAuth
//! sign-in flow works correctly. Tests exercise whichever providers were
//! compiled in at generation time (`oauth_*` template options) via the first
//! configured provider — every provider shares the same generic handlers.

use crate::config::OAuthProviderConfig;
use crate::tests::{AnonymousUser, CookieUser, RequestHelper, TestApp};
use crate::Env;
use http::StatusCode;

/// Returns the first configured OAuth provider, if any were compiled in.
/// Tests using this helper return early on renders with no providers.
fn test_provider(app: &TestApp) -> Option<&OAuthProviderConfig> {
    app.config.oauth_providers.first()
}

fn authorize_uri(slug: &str) -> String {
    format!("/api/v1/auth/{slug}/authorize")
}

fn callback_uri(slug: &str, query: &str) -> String {
    format!("/api/v1/auth/{slug}/callback{query}")
}

#[tokio::test]
async fn oauth_authorize_redirects_to_provider() {
    let app = TestApp::new().await;
    let Some(provider) = test_provider(&app).map(|p| (p.spec.slug, p.spec.authorize_url)) else {
        return;
    };
    let (slug, authorize_url) = provider;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>(&format!("{}?redirect_to=/dashboard", authorize_uri(slug)))
        .await;

    response.assert_status(StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.starts_with(authorize_url));
    assert!(location.contains(&format!("client_id=test_{slug}_client_id")));
}

#[tokio::test]
async fn oauth_authorize_without_redirect_to_uses_default() {
    let app = TestApp::new().await;
    let Some(provider) = test_provider(&app).map(|p| (p.spec.slug, p.spec.authorize_url)) else {
        return;
    };
    let (slug, authorize_url) = provider;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>(&authorize_uri(slug)).await;

    response.assert_status(StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.starts_with(authorize_url));
}

#[tokio::test]
async fn oauth_authorize_unknown_provider_returns_error() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/v1/auth/nosuch/authorize").await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_callback_unknown_provider_returns_error() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>("/api/v1/auth/nosuch/callback?code=test&state=test")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_callback_without_state_returns_error() {
    let app = TestApp::new().await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>(&callback_uri(slug, "?code=test_code")).await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_callback_without_code_returns_error() {
    let app = TestApp::new().await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<serde_json::Value>(&callback_uri(slug, "?state=test_state"))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_callback_with_error_param_returns_controlled_response() {
    let app = TestApp::new().await;
    let Some(provider) = test_provider(&app).map(|p| (p.spec.slug, p.spec.display_name)) else {
        return;
    };
    let (slug, display_name) = provider;
    let anon = AnonymousUser::new(app);

    // Providers redirect here without code/state when the user declines
    // authorization: ?error=access_denied&error_description=...
    let response = anon
        .get::<serde_json::Value>(&callback_uri(
            slug,
            "?error=access_denied&error_description=denied",
        ))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);

    let body = response.into_string().await;
    assert!(
        body.contains(&format!("{display_name} authorization was declined")),
        "Expected friendly 'declined' message but got: {body}"
    );
}

#[tokio::test]
async fn oauth_callback_with_invalid_state_returns_error() {
    let app = TestApp::new().await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    // Call authorize to set up a session with a valid OAuth state
    let auth_response = anon.get::<()>(&authorize_uri(slug)).await;
    auth_response.assert_status(StatusCode::SEE_OTHER);

    // Store the session cookie
    let set_cookie = auth_response
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .expect("No Set-Cookie header from authorize");
    anon.update_session_cookie(set_cookie.to_string());

    // Call callback with a state that doesn't match the one stored in session
    let response = anon
        .get::<()>(&callback_uri(slug, "?code=test_code&state=wrong_state"))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);

    let body = response.into_string().await;
    assert!(
        body.contains("Invalid OAuth state"),
        "Expected 'Invalid OAuth state' but got: {}",
        body
    );
}

/// A state issued while starting a flow on one provider must not complete a
/// callback on another provider — the session records which provider the
/// state belongs to.
#[tokio::test]
async fn oauth_callback_rejects_provider_mismatch() {
    let app = TestApp::new().await;
    let mut providers = app.config.oauth_providers.iter();
    let (Some(first), Some(second)) = (providers.next(), providers.next()) else {
        // Needs two compiled-in providers; nothing to test otherwise.
        return;
    };
    let (first_slug, second_slug) = (first.spec.slug, second.spec.slug);
    let anon = AnonymousUser::new(app);

    let auth_response = anon.get::<()>(&authorize_uri(first_slug)).await;
    auth_response.assert_status(StatusCode::SEE_OTHER);

    let set_cookie = auth_response
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .expect("No Set-Cookie header from authorize");
    anon.update_session_cookie(set_cookie.to_string());

    // Replay the first provider's flow against the second provider's
    // callback endpoint.
    let response = anon
        .get::<()>(&callback_uri(second_slug, "?code=test_code&state=whatever"))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);

    let body = response.into_string().await;
    assert!(
        body.contains("provider mismatch"),
        "Expected 'provider mismatch' but got: {}",
        body
    );
}

#[tokio::test]
async fn login_page_lists_configured_providers() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/login").await;
    response.assert_status(StatusCode::OK);

    let body = response.into_string().await;
    // Every compiled-in provider has credentials in test config, so each one
    // renders a sign-in link.
    for spec in crate::oauth::provider_specs() {
        assert!(
            body.contains(&format!("/api/v1/auth/{}/authorize", spec.slug)),
            "Expected sign-in link for {} in login page",
            spec.slug
        );
    }
}

#[tokio::test]
async fn logout_clears_session() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();
    let user = app
        .user_builder("logout_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);

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

#[tokio::test]
async fn protected_route_requires_session() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Try to access a protected route without session
    let response = anon
        .post::<serde_json::Value>("/api/v1/tokens", &[] as &[u8])
        .await;

    // Should return 401 or 403 since no session exists
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn session_persists_across_requests() {
    let app = TestApp::new().await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    // First request to authorize
    let auth_response = anon.get::<()>(&authorize_uri(slug)).await;
    auth_response.assert_status(StatusCode::SEE_OTHER);

    // Extract and store the session cookie
    let set_cookie = auth_response
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .expect("No Set-Cookie header");
    anon.update_session_cookie(set_cookie.to_string());

    // Second request should have the session cookie
    let response = anon
        .get::<()>(&callback_uri(slug, "?code=test&state=test"))
        .await;
    // Should get BAD_REQUEST because state doesn't match, but session should be present
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn oauth_callback_rejects_malformed_code() {
    let app = TestApp::new().await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    // Call authorize first to set up session
    let _ = anon.get::<()>(&authorize_uri(slug)).await;

    // Call callback with malformed code
    let response = anon
        .get::<()>(&callback_uri(slug, "?code=&state=test"))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn forged_unsigned_session_cookie_is_rejected() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Build session data map with a forged user_id
    let mut map = std::collections::HashMap::new();
    map.insert("user_id".to_string(), "1".to_string());

    // Encode the session data
    let encoded = crate::middleware::session::encode(&map);

    // Create an UNSIGNED cookie (no signature)
    let cookie = cookie::Cookie::build((crate::middleware::session::COOKIE_NAME, encoded))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .max_age(cookie::time::Duration::days(90))
        .build();

    // Update the anonymous user with the forged unsigned cookie
    anon.update_session_cookie(cookie.to_string());

    // Try to access a protected route with the forged cookie
    let response = anon
        .post::<serde_json::Value>("/api/v1/tokens", &[] as &[u8])
        .await;

    // Should return 401 because the unsigned cookie is rejected
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn session_cookie_has_required_security_flags_in_production() {
    let mut config = TestApp::test_config();
    config.base.env = Env::Production;
    let app = TestApp::with_config(config).await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>(&authorize_uri(slug)).await;
    response.assert_status(StatusCode::SEE_OTHER);

    let set_cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .expect("No Set-Cookie header");
    assert!(
        set_cookie.contains("HttpOnly"),
        "Expected HttpOnly: {set_cookie}"
    );
    assert!(
        set_cookie.contains("Secure"),
        "Expected Secure: {set_cookie}"
    );
    assert!(
        set_cookie.contains("SameSite=Lax"),
        "Expected SameSite=Lax: {set_cookie}"
    );
    assert!(
        set_cookie.contains("Max-Age=7776000"),
        "Expected Max-Age=7776000: {set_cookie}"
    );
}

#[tokio::test]
async fn session_cookie_omits_secure_in_development() {
    let mut config = TestApp::test_config();
    config.base.env = Env::Development;
    let app = TestApp::with_config(config).await;
    let Some(slug) = test_provider(&app).map(|p| p.spec.slug) else {
        return;
    };
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>(&authorize_uri(slug)).await;
    response.assert_status(StatusCode::SEE_OTHER);

    let set_cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .expect("No Set-Cookie header");
    assert!(
        set_cookie.contains("HttpOnly"),
        "Expected HttpOnly: {set_cookie}"
    );
    assert!(
        !set_cookie.contains("Secure"),
        "Should not contain Secure in development: {set_cookie}"
    );
    assert!(
        set_cookie.contains("SameSite=Lax"),
        "Expected SameSite=Lax: {set_cookie}"
    );
    assert!(
        set_cookie.contains("Max-Age=7776000"),
        "Expected Max-Age=7776000: {set_cookie}"
    );
}

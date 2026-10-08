//! Middleware authentication and CSRF tests
//!
//! Tests for the split middleware: csrf_protect, require_session_user, and require_api_token.

use crate::tests::{AnonymousUser, CookieUser, RequestHelper, TestApp};
use http::StatusCode;
use jiff::SignedDuration;
use tower::ServiceExt;

/// Operational endpoints live outside the rate-limited route subtree, so
/// health probes and asset requests must never consume rate-limit tokens —
/// even with a tiny burst allowance on every action.
#[tokio::test]
async fn operational_routes_are_never_rate_limited() {
    use crate::rate_limiter::{LimitedAction, RateLimiterConfig};

    let mut config = TestApp::test_config();
    for action in LimitedAction::VARIANTS {
        config.rate_limiter_config.insert(
            action,
            RateLimiterConfig {
                rate: std::time::Duration::from_secs(60),
                burst: 1,
            },
        );
    }

    let app = TestApp::with_config(config).await;
    let anon = AnonymousUser::new(app);

    // Far more requests than any burst allowance — none may be limited.
    for _ in 0..15 {
        let response = anon.get::<()>("/health").await;
        response.assert_status(StatusCode::OK);

        let response = anon.get::<()>("/static/vendor/htmx.min.js").await;
        response.assert_status(StatusCode::OK);
    }
}

/// Anonymous page and asset requests must not create a session cookie: sessions
/// are only persisted when they carry state (login, CSRF token for an
/// authenticated session, OAuth handshake).
#[tokio::test]
async fn anonymous_requests_do_not_set_cookies() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/").await;
    response.assert_status(StatusCode::OK);
    assert!(
        response.headers().get("set-cookie").is_none(),
        "anonymous GET / must not emit Set-Cookie"
    );

    let response = anon.get::<()>("/static/vendor/htmx.min.js").await;
    response.assert_status(StatusCode::OK);
    assert!(
        response.headers().get("set-cookie").is_none(),
        "anonymous static requests must not emit Set-Cookie"
    );
}

/// CORS is the outermost middleware layer: a browser preflight must be answered
/// without a User-Agent, cookies, or any rate-limit budget. To prove no token
/// is consumed, all actions are configured with a zero-length burst.
#[tokio::test]
async fn cors_preflight_needs_no_user_agent_or_session() {
    use crate::rate_limiter::{LimitedAction, RateLimiterConfig};

    let mut config = TestApp::test_config();
    for action in LimitedAction::VARIANTS {
        config.rate_limiter_config.insert(
            action,
            RateLimiterConfig {
                rate: std::time::Duration::from_secs(60),
                burst: 0,
            },
        );
    }

    let app = TestApp::with_config(config).await;

    for _ in 0..3 {
        let mut request = axum::extract::Request::builder()
            .method(http::Method::OPTIONS)
            .uri("/api/v1/tokens")
            .header("origin", "http://localhost:3000")
            .header("access-control-request-method", "POST")
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

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "preflight must succeed even with zero rate-limit budget"
        );
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|h| h.to_str().ok()),
            Some("http://localhost:3000")
        );
        assert!(
            response.headers().get("set-cookie").is_none(),
            "preflight must not emit Set-Cookie"
        );
    }
}

/// Public read-only API routes are rate limited per client IP: anonymous
/// reads of the open API are rejected once the `PublicApiRead` burst is
/// exhausted. (Operational routes like `/health` stay exempt — covered by
/// `operational_routes_are_never_rate_limited` above.)
#[tokio::test]
async fn public_api_reads_are_rate_limited() {
    use crate::rate_limiter::{LimitedAction, RateLimiterConfig};

    let mut config = TestApp::test_config();
    config.rate_limiter_config.insert(
        LimitedAction::PublicApiRead,
        RateLimiterConfig {
            rate: std::time::Duration::from_secs(60),
            burst: 1,
        },
    );

    let app = TestApp::with_config(config).await;
    let anon = AnonymousUser::new(app);

    // burst = 1 allows a single read; subsequent reads are rejected.
    let response = anon.get::<serde_json::Value>("/api/v1/posts").await;
    response.assert_status(StatusCode::OK);

    let response = anon.get::<serde_json::Value>("/api/v1/posts").await;
    response.assert_status(StatusCode::TOO_MANY_REQUESTS);

    // The bucket is shared by every public read route, including show-by-id.
    let response = anon.get::<serde_json::Value>("/api/v1/posts/1").await;
    response.assert_status(StatusCode::TOO_MANY_REQUESTS);
}

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
    let Some(slug) = app
        .config
        .oauth_providers
        .first()
        .map(|provider| provider.spec.slug)
    else {
        // No OAuth providers were compiled in — nothing to exercise.
        return;
    };
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<()>(&format!("/api/v1/auth/{slug}/authorize"))
        .await;

    response.assert_status(StatusCode::SEE_OTHER);
}

/// A session carrying `user_id` but no CSRF token must NOT bypass CSRF
/// validation on unsafe methods — regression test for the bypass where
/// `csrf_protect` only validated sessions that already held a token.
#[tokio::test]
async fn csrf_protected_route_with_session_but_no_csrf_returns_error() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();
    let user = app
        .user_builder("csrfless_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    // Fabricate a session that has user_id but no csrf_token.
    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    let response = cookie_user
        .post::<serde_json::Value>(
            "/api/v1/tokens",
            serde_json::json!({ "name": "test-token" }),
        )
        .await;

    // Authentication passes (the user exists); CSRF rejects the request.
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "session with user_id but no csrf_token must be rejected"
    );
}

#[tokio::test]
async fn csrf_protected_route_with_valid_csrf_succeeds() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();
    let user = app
        .user_builder("csrf_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    // Creates the CSRF token in the session and returns the new cookie.
    let csrf_token = cookie_user.init_csrf().await;

    // Token routes should succeed with valid CSRF token
    let response = cookie_user
        .post_with_headers::<serde_json::Value>(
            "/api/v1/tokens",
            serde_json::json!({ "name": "test-token" }),
            cookie_user.headers_with_csrf(&csrf_token),
        )
        .await;

    response.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn malformed_json_returns_error() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();
    let user = app
        .user_builder("malformed_json_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    // Send malformed JSON
    let response = cookie_user
        .post_with_headers::<serde_json::Value>(
            "/api/v1/tokens",
            &b"{invalid json}"[..],
            cookie_user.headers_with_csrf(&csrf_token),
        )
        .await;

    // Should return a client error (400 or 422)
    assert!(response.status().is_client_error());
}

/// HTMX requests must still receive partial templates — the `REQUEST_FORMAT`
/// task-local is scoped by a dedicated middleware, independent of the error
/// logging layer.
#[tokio::test]
async fn htmx_requests_render_partial_templates() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Full page by default
    let response = anon.get::<()>("/api/server-time").await;
    response.assert_status(StatusCode::OK);
    let body = response.into_string().await;
    assert!(body.contains("<main"), "expected full page, got: {body}");

    // HTMX request renders the partial fragment only
    let mut request = anon.request_builder(http::Method::GET, "/api/server-time");
    request
        .headers_mut()
        .insert("hx-request", "true".parse().unwrap());
    let response = anon.run::<()>(request).await;
    response.assert_status(StatusCode::OK);
    let body = response.into_string().await;
    assert!(
        body.contains("time-response"),
        "expected partial fragment, got: {body}"
    );
    assert!(
        !body.contains("<main"),
        "HTMX request must not render the full page"
    );
}

#[tokio::test]
async fn locked_user_with_valid_session_is_forbidden() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("locked_session_user")
        .locked(
            "Account is locked",
            Some(
                jiff::Timestamp::now()
                    .checked_add(SignedDuration::from_hours(1))
                    .unwrap(),
            ),
        )
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    let response = cookie_user.get::<serde_json::Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn inactive_user_with_valid_session_is_forbidden() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("inactive_session_user")
        .inactive()
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    let response = cookie_user.get::<serde_json::Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::FORBIDDEN);
}

/// Account locks are enforced lazily, only where authentication is required.
/// A locked user may still browse public pages — matching crates.io's model
/// where `AuthCheck` runs per-route.
#[tokio::test]
async fn locked_user_can_access_public_routes() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("locked_public_user")
        .locked(
            "Account is locked",
            Some(
                jiff::Timestamp::now()
                    .checked_add(SignedDuration::from_hours(1))
                    .unwrap(),
            ),
        )
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    let response = cookie_user.get::<()>("/").await;
    response.assert_status(StatusCode::OK);

    let response = cookie_user.get::<serde_json::Value>("/api/v1/posts").await;
    response.assert_status(StatusCode::OK);
}

/// Deactivated users are rejected only where authentication is enforced —
/// like locked users, they may still browse public pages.
#[tokio::test]
async fn inactive_user_can_access_public_routes() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("inactive_public_user")
        .inactive()
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    let response = cookie_user.get::<()>("/").await;
    response.assert_status(StatusCode::OK);

    let response = cookie_user.get::<serde_json::Value>("/api/v1/posts").await;
    response.assert_status(StatusCode::OK);
}

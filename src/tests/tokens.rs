//! Token CRUD integration tests
//!
//! Tests all 3 token endpoints: create, list, revoke.

use crate::models::ActionScope;
use crate::tests::{CookieUser, RequestHelper, TestApp, TokenUser};
use http::StatusCode;
use jiff::SignedDuration;
use serde_json::{json, Value};

/// Helper to set up an authenticated user with CSRF token.
/// Returns (cookie_user, csrf_token, user_id).
async fn setup_auth_user() -> (CookieUser, String, u64) {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("token_test_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let user_id = user.id;
    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user_id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    (cookie_user, csrf_token, user_id)
}

#[tokio::test]
async fn create_token_returns_201() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            "/api/v1/tokens",
            &json!({
                "name": "my-api-token"
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::CREATED);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["name"], "my-api-token");
    assert!(
        data["token"].as_str().unwrap().starts_with("ako"),
        "Token should start with 'ako' prefix"
    );
}

#[tokio::test]
async fn create_token_with_empty_name_returns_400() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            "/api/v1/tokens",
            &json!({
                "name": "  "
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_token_with_scopes_returns_201() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            "/api/v1/tokens",
            &json!({
                "name": "scoped-token",
                "resource_scopes": ["posts*"],
                "action_scopes": ["read", "create"]
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::CREATED);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["name"], "scoped-token");
    let resource_scopes = data["resource_scopes"].as_array().expect("resource_scopes");
    assert!(resource_scopes.iter().any(|s| s == "posts*"));
    let action_scopes = data["action_scopes"].as_array().expect("action_scopes");
    assert!(action_scopes.iter().any(|s| s == "read"));
    assert!(action_scopes.iter().any(|s| s == "create"));
}

#[tokio::test]
async fn list_tokens_returns_user_tokens() {
    let (cookie_user, _csrf_token, user_id) = setup_auth_user().await;

    // Insert tokens directly into DB
    let mut db = cookie_user.app().db().db_clone();
    cookie_user
        .app()
        .token_builder(user_id, "token-1")
        .build(&mut db)
        .await
        .expect("Failed to create token 1");
    cookie_user
        .app()
        .token_builder(user_id, "token-2")
        .build(&mut db)
        .await
        .expect("Failed to create token 2");

    let response = cookie_user.get::<Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = body["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 2);
}

#[tokio::test]
async fn list_tokens_only_returns_own_tokens() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user1 = app
        .user_builder("user1")
        .build(&mut db)
        .await
        .expect("Failed to create user1");
    let user2 = app
        .user_builder("user2")
        .build(&mut db)
        .await
        .expect("Failed to create user2");

    app.token_builder(user1.id, "user1-token")
        .build(&mut db)
        .await
        .expect("Failed to create token for user1");
    app.token_builder(user2.id, "user2-token")
        .build(&mut db)
        .await
        .expect("Failed to create token for user2");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user1.id, session_key);
    let _ = cookie_user.init_csrf().await;

    let response = cookie_user.get::<Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = body["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], "user1-token");
}

#[tokio::test]
async fn revoke_token_returns_204() {
    let (cookie_user, csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let (token, _plain) = cookie_user
        .app()
        .token_builder(user_id, "revoke-me")
        .build(&mut db)
        .await
        .expect("Failed to create token");

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            &format!("/api/v1/tokens/{}", token.id),
            &[] as &[u8],
            headers,
        )
        .await;

    response.assert_status(StatusCode::NO_CONTENT);

    // Verify the token is revoked in the list
    let response = cookie_user.get::<Value>("/api/v1/tokens").await;
    let body = response.into_json::<Value>().await;
    let data = body["data"].as_array().expect("data should be an array");
    let revoked_token = data
        .iter()
        .find(|t| t["id"] == token.id)
        .expect("Token should still be in list");
    assert_eq!(revoked_token["revoked"], true);
}

#[tokio::test]
async fn revoke_token_returns_404_for_nonexistent() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>("/api/v1/tokens/99999", &[] as &[u8], headers)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn revoke_token_returns_404_for_other_users_token() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user1 = app
        .user_builder("owner")
        .build(&mut db)
        .await
        .expect("Failed to create owner");
    let user2 = app
        .user_builder("revoker")
        .build(&mut db)
        .await
        .expect("Failed to create revoker");

    let (token, _plain) = app
        .token_builder(user1.id, "owner-token")
        .build(&mut db)
        .await
        .expect("Failed to create token");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user2.id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            &format!("/api/v1/tokens/{}", token.id),
            &[] as &[u8],
            headers,
        )
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tokens_require_authentication() {
    let app = TestApp::new().await;
    let anon = crate::tests::AnonymousUser::new(app);

    let response = anon
        .post::<Value>("/api/v1/tokens", &json!({"name": "test"}))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_tokens_requires_authentication() {
    let app = TestApp::new().await;
    let anon = crate::tests::AnonymousUser::new(app);

    let response = anon.get::<Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_token_without_csrf_returns_error() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("no_csrf_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);
    let _ = cookie_user.init_csrf().await;

    // POST without CSRF token header
    let response = cookie_user
        .post::<Value>("/api/v1/tokens", &json!({"name": "test"}))
        .await;

    assert!(response.status().is_client_error());
    assert_ne!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn token_auth_can_list_tokens_without_csrf() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("token_auth_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let (_api_token, plain_token) = app
        .token_builder(user.id, "read-token")
        .action_scopes(vec![ActionScope::Read])
        .build(&mut db)
        .await
        .expect("Failed to create token");

    let token_user = TokenUser::new(app, plain_token);

    let response = token_user.get::<Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = body["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], "read-token");
}

#[tokio::test]
async fn token_auth_without_required_scope_is_forbidden() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("token_auth_user_no_scope")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    // Token with only "create" action scope cannot list tokens
    let (_api_token, plain_token) = app
        .token_builder(user.id, "create-token")
        .action_scopes(vec![ActionScope::Create])
        .build(&mut db)
        .await
        .expect("Failed to create token");

    let token_user = TokenUser::new(app, plain_token);

    let response = token_user.get::<Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn token_auth_for_locked_user_is_forbidden() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("locked_token_user")
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

    let (_api_token, plain_token) = app
        .token_builder(user.id, "locked-user-token")
        .action_scopes(vec![ActionScope::Read])
        .build(&mut db)
        .await
        .expect("Failed to create token");

    let token_user = TokenUser::new(app, plain_token);

    let response = token_user.get::<Value>("/api/v1/tokens").await;

    response.assert_status(StatusCode::FORBIDDEN);
}

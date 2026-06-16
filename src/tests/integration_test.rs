//! Integration tests demonstrating the test infrastructure
//!
//! This file shows how to use the adapted test infrastructure
//! from crates.io for writing integration tests.

use axum_kickoff::tests::{AnonymousUser, TestApp};
use http::StatusCode;

#[tokio::test]
async fn test_health_check() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_home_page() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/").await;
    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_server_time() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/api/server-time").await;
    response.assert_status(StatusCode::OK);

    let json = response.into_json::<serde_json::Value>().await;
    assert!(json.is_object());
}

#[tokio::test]
async fn test_not_found() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/nonexistent").await;
    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_response_assertions() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    // Chain assertions
    response
        .assert_status(StatusCode::OK)
        .assert_success();
}

#[tokio::test]
async fn test_response_body_string() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    let body = response.into_string().await;

    assert_eq!(body, "OK");
}

#[tokio::test]
async fn test_response_content_type() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;

    // Health endpoint returns text/plain
    let content_type = response.content_type();
    assert!(content_type.is_some());
}

#[tokio::test]
async fn test_builders_and_database() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    // 1. Build and insert the user
    let user = app
        .user_builder("mona_lisa")
        .email("octocat@github.com")
        .build(&mut db)
        .await
        .expect("Failed to insert user");

    assert_eq!(user.gh_login, "mona_lisa");

    // 2. Build and insert a token for the user
    let (token, plain_token) = app
        .token_builder(user.id, "my-test-token")
        .build(&mut db)
        .await
        .expect("Failed to insert token");

    assert_eq!(token.name, "my-test-token");
    assert_eq!(token.user_id, user.id);
    assert!(plain_token.starts_with("ako"));
}

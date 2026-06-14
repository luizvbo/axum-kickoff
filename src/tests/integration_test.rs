//! Integration tests demonstrating the test infrastructure
//!
//! This file shows how to use the adapted test infrastructure
//! from crates.io for writing integration tests.

use axum_kickoff::tests::{AnonymousUser, TestApp};
use http::StatusCode;

#[tokio::test]
async fn test_health_check() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_home_page() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/").await;
    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_server_time() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/api/server-time").await;
    response.assert_status(StatusCode::OK);
    
    let json = response.into_json::<serde_json::Value>().await;
    assert!(json.is_object());
}

#[tokio::test]
async fn test_not_found() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/nonexistent").await;
    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_response_assertions() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    
    // Chain assertions
    response
        .assert_status(StatusCode::OK)
        .assert_success();
}

#[tokio::test]
async fn test_response_body_string() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    let body = response.into_string().await;
    
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn test_response_content_type() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    
    // Health endpoint returns text/plain
    let content_type = response.content_type();
    assert!(content_type.is_some());
}

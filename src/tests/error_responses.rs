//! Error response integration tests
//!
//! Adapted from crates.io's error response tests to verify that
//! the application returns proper error responses for various scenarios.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};

#[tokio::test]
async fn visiting_unknown_route_returns_404() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/does-not-exist").await;
    response.assert_status(http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn visiting_unknown_api_route_returns_404() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/v1/does-not-exist").await;
    response.assert_status(http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn health_endpoint_returns_200() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(http::StatusCode::OK);
}

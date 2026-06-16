//! Error response integration tests
//!
//! Verifies that the application returns proper error responses for various scenarios,
//! with snapshot assertions to track exact JSON outputs.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};

#[tokio::test]
async fn visiting_unknown_route_returns_404() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/does-not-exist").await;
    response.assert_status(http::StatusCode::NOT_FOUND);

    let json = response.into_json::<serde_json::Value>().await;
    insta::assert_json_snapshot!(json, @r###"
    {
      "detail": "Not Found"
    }
    "###);
}

#[tokio::test]
async fn visiting_unknown_api_route_returns_404() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon
        .get::<serde_json::Value>("/api/v1/does-not-exist")
        .await;
    response.assert_status(http::StatusCode::NOT_FOUND);

    let json = response.into_json::<serde_json::Value>().await;
    insta::assert_json_snapshot!(json, @r###"
    {
      "detail": "Not Found"
    }
    "###);
}

#[tokio::test]
async fn health_endpoint_returns_200() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(http::StatusCode::OK);
}

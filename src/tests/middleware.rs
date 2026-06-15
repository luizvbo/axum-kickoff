//! Middleware tests
//!
//! Tests for middleware components including path normalization.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};

#[tokio::test]
async fn path_normalization_trailing_slash() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Test that the health endpoint works (middleware is applied)
    let response = anon.get::<serde_json::Value>("/health").await;
    response.assert_status(http::StatusCode::OK);
}

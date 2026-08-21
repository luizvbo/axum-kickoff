//! Error response integration tests
//!
//! Verifies that the application returns proper error responses for various scenarios,
//! with snapshot assertions to track exact JSON and HTML outputs.

use crate::tests::{AnonymousUser, RequestHelper, TestApp};
use axum::body::Body;
use http::{header, HeaderValue, Method};
use regex::Regex;

fn sanitize_html(html: &str) -> String {
    let nonce = Regex::new(r#"nonce="[A-Za-z0-9+/=]{20,26}""#).unwrap();
    nonce
        .replace_all(html, r#"nonce="[CSP_NONCE]""#)
        .into_owned()
}

fn html_request(anon: &AnonymousUser, path: &str) -> http::Request<Body> {
    let mut request = anon.request_builder(Method::GET, path);
    request.headers_mut().insert(
        header::ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml"),
    );
    request
}

#[tokio::test]
async fn visiting_unknown_route_returns_404() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/does-not-exist").await;
    response.assert_status(http::StatusCode::NOT_FOUND);

    let json = response.into_json::<serde_json::Value>().await;
    insta::assert_json_snapshot!(json);
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
    insta::assert_json_snapshot!(json);
}

#[tokio::test]
async fn html_error_page_snapshot() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let request = html_request(&anon, "/does-not-exist");
    let response = anon.run::<String>(request).await;
    response.assert_status(http::StatusCode::NOT_FOUND);

    let body = sanitize_html(&response.into_string().await);
    insta::assert_snapshot!(body);
}

#[tokio::test]
async fn unauthorized_api_error_snapshot() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/api/v1/tokens").await;
    response.assert_status(http::StatusCode::UNAUTHORIZED);

    let json = response.into_json::<serde_json::Value>().await;
    insta::assert_json_snapshot!(json);
}

#[tokio::test]
async fn health_endpoint_returns_200() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(http::StatusCode::OK);
}

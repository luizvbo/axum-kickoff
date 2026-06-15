//! Request helper trait for making authenticated requests
//!
//! Adapted from crates.io's RequestHelper to provide ergonomic
//! request building for different authentication states.

use crate::tests::response::Response;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{HeaderMap, HeaderValue, Method, Request, Uri};
use http::header;
use serde::Serialize;
use std::net::SocketAddr;
use tower::ServiceExt;

/// Trait for making HTTP requests in tests
///
/// This trait is implemented by different authentication states
/// (anonymous, cookie-based, token-based) to provide a consistent
/// interface for making requests.
#[allow(async_fn_in_trait)]
pub trait RequestHelper {
    /// Get the test app reference
    fn app(&self) -> &super::test_app::TestApp;

    /// Get the headers to include in requests
    fn headers(&self) -> HeaderMap;

    /// Build a request with the given method and path
    fn request_builder(&self, method: Method, path: &str) -> Request<Body> {
        let uri = Uri::builder()
            .path_and_query(path)
            .build()
            .expect("Invalid URI");

        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::USER_AGENT, "axum-kickoff-test")
            .body(Body::empty())
            .expect("Failed to build request");

        // Add headers from the authentication state
        for (name, value) in self.headers().iter() {
            request.headers_mut().insert(name, value.clone());
        }

        request
    }

    /// Run a request that is expected to succeed
    async fn run<T>(&self, request: Request<impl Into<Body>>) -> Response<T> {
        let app = self.app();
        let request = request.map(Into::into);

        // Add mock connection info
        let mut request = request;
        request
            .extensions_mut()
            .insert(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))));

        let response = app
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to execute request");

        Response::new(response)
    }

    /// Make a GET request
    async fn get<T>(&self, path: &str) -> Response<T> {
        let request = self.request_builder(Method::GET, path);
        self.run(request).await
    }

    /// Make a POST request with a JSON body
    async fn post<T>(&self, path: &str, body: impl Serialize) -> Response<T> {
        let json_body = serde_json::to_string(&body).expect("Failed to serialize body");
        
        let mut request = self.request_builder(Method::POST, path);
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Body::from(json_body);

        self.run(request).await
    }

    /// Make a PUT request with a JSON body
    async fn put<T>(&self, path: &str, body: impl Serialize) -> Response<T> {
        let json_body = serde_json::to_string(&body).expect("Failed to serialize body");
        
        let mut request = self.request_builder(Method::PUT, path);
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Body::from(json_body);

        self.run(request).await
    }

    /// Make a DELETE request
    async fn delete<T>(&self, path: &str) -> Response<T> {
        let request = self.request_builder(Method::DELETE, path);
        self.run(request).await
    }

    /// Make a PATCH request with a JSON body
    async fn patch<T>(&self, path: &str, body: impl Serialize) -> Response<T> {
        let json_body = serde_json::to_string(&body).expect("Failed to serialize body");
        
        let mut request = self.request_builder(Method::PATCH, path);
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Body::from(json_body);

        self.run(request).await
    }
}

/// Anonymous user (no authentication)
pub struct AnonymousUser {
    app: super::test_app::TestApp,
}

impl AnonymousUser {
    /// Create a new anonymous user
    pub fn new(app: super::test_app::TestApp) -> Self {
        Self { app }
    }
}

impl RequestHelper for AnonymousUser {
    fn app(&self) -> &super::test_app::TestApp {
        &self.app
    }

    fn headers(&self) -> HeaderMap {
        HeaderMap::new()
    }
}

/// User authenticated via session cookie
pub struct CookieUser {
    app: super::test_app::TestApp,
    user_id: i32,
    session_key: cookie::Key,
}

impl CookieUser {
    /// Create a new cookie-authenticated user
    pub fn new(app: super::test_app::TestApp, user_id: i32, session_key: cookie::Key) -> Self {
        Self {
            app,
            user_id,
            session_key,
        }
    }

    /// Get the user ID
    pub fn user_id(&self) -> i32 {
        self.user_id
    }
}

impl RequestHelper for CookieUser {
    fn app(&self) -> &super::test_app::TestApp {
        &self.app
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        // Encode session cookie
        let cookie = encode_session_header(&self.session_key, self.user_id);
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie).expect("Invalid cookie header"),
        );

        headers
    }
}

/// User authenticated via API token
pub struct TokenUser {
    app: super::test_app::TestApp,
    token: String,
}

impl TokenUser {
    /// Create a new token-authenticated user
    pub fn new(app: super::test_app::TestApp, token: String) -> Self {
        Self { app, token }
    }

    /// Get the API token
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl RequestHelper for TokenUser {
    fn app(&self) -> &super::test_app::TestApp {
        &self.app
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        let auth_value = format!("Bearer {}", self.token);
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&auth_value).expect("Invalid auth header"),
        );

        headers
    }
}

/// Encode a session cookie header for mock requests
///
/// This matches the session encoding used in the session middleware.
pub fn encode_session_header(session_key: &cookie::Key, user_id: i32) -> String {
    let cookie_name = "cargo_session";

    // Build session data map
    let mut map = std::collections::HashMap::new();
    map.insert("user_id".to_string(), user_id.to_string());

    // Encode the map into a cookie value string
    // Note: This is a simplified version - the actual session encoding
    // would use the session middleware's encoding logic
    let encoded = serde_json::to_string(&map).expect("Failed to encode session");

    // Put the cookie into a signed cookie jar
    let cookie = cookie::Cookie::build((cookie_name, encoded));
    let mut jar = cookie::CookieJar::new();
    jar.signed_mut(session_key).add(cookie);

    // Read the raw cookie from the cookie jar
    jar.get(cookie_name).unwrap().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_session_header() {
        let session_key = cookie::Key::generate();
        let cookie = encode_session_header(&session_key, 42);
        assert!(cookie.contains("cargo_session="));
    }
}

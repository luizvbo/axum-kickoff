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
use std::sync::{Arc, Mutex};
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

    /// Make a POST request with custom headers
    async fn post_with_headers<T>(
        &self,
        path: &str,
        body: impl Serialize,
        headers: HeaderMap,
    ) -> Response<T> {
        let json_body = serde_json::to_string(&body).expect("Failed to serialize body");

        let mut request = self.request_builder(Method::POST, path);
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Body::from(json_body);

        // Add custom headers
        for (name, value) in headers.iter() {
            request.headers_mut().insert(name, value.clone());
        }

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
    /// Stored Set-Cookie header value for cookie persistence
    session_cookie: Arc<Mutex<Option<String>>>,
}

impl AnonymousUser {
    /// Create a new anonymous user
    pub fn new(app: super::test_app::TestApp) -> Self {
        Self {
            app,
            session_cookie: Arc::new(Mutex::new(None)),
        }
    }

    /// Update the stored session cookie from a response
    pub fn update_session_cookie(&self, set_cookie_value: String) {
        *self.session_cookie.lock().unwrap() = Some(set_cookie_value);
    }
}

impl RequestHelper for AnonymousUser {
    fn app(&self) -> &super::test_app::TestApp {
        &self.app
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // Add stored session cookie if available
        if let Some(cookie) = self.session_cookie.lock().unwrap().as_ref() {
            if let Ok(value) = HeaderValue::from_str(cookie) {
                headers.insert(header::COOKIE, value);
            }
        }

        headers
    }
}

/// User authenticated via session cookie
pub struct CookieUser {
    app: super::test_app::TestApp,
    user_id: i32,
    session_key: cookie::Key,
    /// Stored Set-Cookie header value for cookie persistence
    session_cookie: Arc<Mutex<Option<String>>>,
}

impl CookieUser {
    /// Create a new cookie-authenticated user
    pub fn new(app: super::test_app::TestApp, user_id: i32, session_key: cookie::Key) -> Self {
        Self {
            app,
            user_id,
            session_key,
            session_cookie: Arc::new(Mutex::new(None)),
        }
    }

    /// Update the stored session cookie from a response
    pub fn update_session_cookie(&self, set_cookie_value: String) {
        *self.session_cookie.lock().unwrap() = Some(set_cookie_value);
    }

    /// Get the user ID
    pub fn user_id(&self) -> i32 {
        self.user_id
    }

    /// Get the CSRF token from the session
    ///
    /// Returns the CSRF token if it exists in the session, otherwise returns None.
    /// This method does NOT generate a new token - tests should create CSRF tokens
    /// by making a real GET request to a route that calls get_or_create_csrf_token.
    pub fn get_csrf_token(&self) -> Option<String> {
        use crate::middleware::session::decode;
        use cookie::{Cookie, CookieJar};

        // Use stored session cookie if available (updated from responses)
        let cookie_str = if let Some(cookie) = self.session_cookie.lock().unwrap().as_ref() {
            cookie.clone()
        } else {
            // Otherwise, encode session cookie from user_id
            encode_session_header(&self.session_key, self.user_id)
        };

        // URL-decode the cookie string (Set-Cookie headers are URL-encoded)
        let cookie_str = urlencoding::decode(&cookie_str).ok()?.into_owned();

        // Parse the cookie string into an owned Cookie
        let cookie = Cookie::parse(cookie_str.clone()).ok()?;
        if cookie.name() != "axum_kickoff_session" {
            return None;
        }

        // Try to verify the signed cookie
        let mut jar = CookieJar::new();
        jar.add_original(Cookie::new(
            cookie.name().to_string(),
            cookie.value().to_string(),
        ));
        let verified_cookie = jar.signed(&self.session_key).get("axum_kickoff_session")?;

        // Decode the verified cookie value
        let session_data = decode(verified_cookie);
        session_data.get("csrf_token").cloned()
    }
}

impl RequestHelper for CookieUser {
    fn app(&self) -> &super::test_app::TestApp {
        &self.app
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // Use stored session cookie if available (updated from responses)
        if let Some(cookie) = self.session_cookie.lock().unwrap().as_ref() {
            if let Ok(value) = HeaderValue::from_str(cookie) {
                headers.insert(header::COOKIE, value);
                return headers;
            }
        }

        // Otherwise, encode session cookie from user_id
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
/// The cookie is signed with the session key to match the verification
/// done by the middleware on every request.
pub fn encode_session_header(session_key: &cookie::Key, user_id: i32) -> String {
    let cookie_name = "axum_kickoff_session";

    // Build session data map
    let mut map = std::collections::HashMap::new();
    map.insert("user_id".to_string(), user_id.to_string());

    // Use the same encoding as the session middleware
    let encoded = crate::middleware::session::encode(&map);

    // Create a signed cookie
    let cookie = cookie::Cookie::build((cookie_name, encoded))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .build();

    // Sign the cookie using the session key
    let mut jar = cookie::CookieJar::new();
    jar.signed_mut(session_key).add(cookie);

    // Get the signed cookie value
    jar.get(cookie_name)
        .expect("Failed to sign cookie")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_session_header() {
        let session_key = cookie::Key::generate();
        let cookie = encode_session_header(&session_key, 42);
        assert!(cookie.contains("axum_kickoff_session="));
    }
}

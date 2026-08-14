//! CSRF (Cross-Site Request Forgery) protection
//!
//! This module provides CSRF protection for form submissions and state-changing requests.
//! It generates per-session CSRF tokens and validates them on unsafe HTTP methods.
//!
//! Supports:
//! - Form field submission: `<input type="hidden" name="csrf_token" value="...">`
//! - Header submission: `X-CSRF-Token: ...` (for HTMX and API clients)
//!
//! # Usage
//!
//! ```ignore
//! use axum::{Form, extract::Extension, response::Html};
//! use crate::middleware::SessionExtension;
//!
//! // In a handler that renders a form:
//! async fn show_form(
//!     Extension(session): Extension<SessionExtension>,
//! ) -> Html<String> {
//!     let csrf_token = crate::middleware::get_or_create_csrf_token(&session);
//!     // csrf_token contains the token to include in your form
//!     Html(format!(
//!         r#"<form method="POST">
//!             <input type="hidden" name="csrf_token" value="{}">
//!             ...
//!         </form>"#,
//!         csrf_token
//!     ))
//! }
//!
//! // Apply CSRF protection middleware to routes that process forms:
//! // .route("/submit", submit.route().layer(axum::middleware::from_fn(
//! //     crate::middleware::csrf::protect
//! // )))
//! ```

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;
use http_body_util::BodyExt;
use rand::distr::Alphanumeric;
use rand::RngExt;
use subtle::ConstantTimeEq;

use crate::config::AllowedOrigins;
use crate::middleware::SessionExtension;
use crate::util::auth::Authentication;
use crate::util::errors::{bad_request, AppResult};

pub static CSRF_TOKEN_KEY: &str = "csrf_token";
pub static CSRF_HEADER_NAME: &str = "x-csrf-token";
pub static CSRF_FORM_FIELD: &str = "csrf_token";

/// Generate a cryptographically secure random CSRF token
pub fn generate_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/// Get or create a CSRF token for the current session
pub fn get_or_create_csrf_token(session: &SessionExtension) -> String {
    if let Some(token) = session.get(CSRF_TOKEN_KEY) {
        return token;
    }

    let token = generate_token();
    session.insert(CSRF_TOKEN_KEY.to_string(), token.clone());
    token
}

/// Validate a CSRF token against the session
pub fn validate_csrf_token(session: &SessionExtension, provided_token: &str) -> AppResult<()> {
    let session_token = session
        .get(CSRF_TOKEN_KEY)
        .ok_or_else(|| bad_request("CSRF token not found in session. Please refresh the page."))?;

    if session_token
        .as_bytes()
        .ct_eq(provided_token.as_bytes())
        .into()
    {
        Ok(())
    } else {
        Err(bad_request(
            "Invalid CSRF token. Please refresh the page and try again.",
        ))
    }
}

/// Extract CSRF token from request headers or form body
///
/// If the token is found in the header, returns it immediately.
/// Otherwise, if the request is form-encoded, reads the body to find the token.
/// Returns (token, optional reconstructed request body bytes).
async fn extract_csrf_token(
    method: &Method,
    headers: &HeaderMap,
    mut req: axum::extract::Request,
) -> (Option<String>, axum::extract::Request) {
    // Only validate unsafe methods
    if !is_unsafe_method(method) {
        return (Some(String::new()), req); // Safe methods don't need CSRF
    }

    // First check header (for HTMX and API clients)
    if let Some(header_value) = headers.get(CSRF_HEADER_NAME) {
        if let Ok(token) = header_value.to_str() {
            return (Some(token.to_string()), req);
        }
    }

    // Then check form field if content-type is form-encoded
    let is_form_encoded = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false);

    if is_form_encoded {
        // Read body bytes
        let body = std::mem::replace(req.body_mut(), Body::empty());
        match body.collect().await {
            Ok(collected) => {
                let bytes = collected.to_bytes();
                let body_str = String::from_utf8_lossy(&bytes);
                let token = extract_csrf_from_form_data(&body_str);
                // Reconstruct the request with the same body bytes
                *req.body_mut() = Body::from(bytes);
                return (token, req);
            }
            Err(_) => {
                return (None, req);
            }
        }
    }

    (None, req)
}

/// Parse CSRF token from URL-encoded form data
fn extract_csrf_from_form_data(form_data: &str) -> Option<String> {
    for pair in form_data.split('&') {
        let parts: Vec<&str> = pair.splitn(2, '=').collect();
        if parts.len() == 2 && parts[0] == CSRF_FORM_FIELD {
            if let Ok(decoded) = urlencoding::decode(parts[1]) {
                return Some(decoded.into_owned());
            }
        }
    }
    None
}

/// Check if the HTTP method is unsafe (requires CSRF protection)
fn is_unsafe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// Verify the Origin header for unsafe methods as defense-in-depth against CSRF.
///
/// Checks the `Origin` header against the allowed origins list.
/// If no `Origin` header is present, falls back to `Referer` header.
/// Requests with no origin/referer are allowed (same-origin browser navigations
/// may omit these headers).
///
/// This is designed to be used as `from_fn_with_state` middleware with `AllowedOrigins`.
pub async fn verify_origin(
    State(allowed_origins): State<AllowedOrigins>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let method = req.method();

    if is_unsafe_method(method) {
        let headers = req.headers();

        // Check Origin header first
        let origin = headers
            .get(axum::http::header::ORIGIN)
            .and_then(|v| v.to_str().ok());

        if let Some(origin) = origin {
            if let Ok(header_value) = axum::http::HeaderValue::from_str(origin) {
                if !allowed_origins.contains(&header_value) {
                    return bad_request(
                        "Origin not allowed. Request blocked by origin verification.",
                    )
                    .response();
                }
            } else {
                return bad_request("Invalid Origin header.").response();
            }
        }
    }

    next.run(req).await
}

/// CSRF protection middleware
///
/// Validates CSRF tokens for unsafe HTTP methods (POST, PUT, PATCH, DELETE).
/// Safe methods (GET, HEAD, OPTIONS) are allowed without CSRF validation.
///
/// This middleware checks for the CSRF token in:
/// 1. The `X-CSRF-Token` header (for HTMX and API clients)
/// 2. The `csrf_token` form field (for traditional form submissions)
///
/// # Note
///
/// For form submissions, this middleware expects the form data to be available
/// in the request body. This works with axum's Form extractor.
/// If no session exists or the session is empty, the request passes through unchanged (for API endpoints).
pub async fn protect(req: axum::extract::Request, next: Next) -> Response {
    let method = req.method().clone();
    let headers = req.headers().clone();

    // API token-authenticated requests do not require CSRF protection
    if is_unsafe_method(&method)
        && req
            .extensions()
            .get::<Authentication>()
            .is_some_and(|auth| auth.is_token())
    {
        return next.run(req).await;
    }

    // Only validate unsafe methods if session exists and has data
    if is_unsafe_method(&method) {
        if let Some(session) = req.extensions().get::<SessionExtension>().cloned() {
            // Only validate if session has actual data (not empty/anonymous)
            if session.get("user_id").is_some() || session.get(CSRF_TOKEN_KEY).is_some() {
                // Extract CSRF token from header or form body
                let (provided_token, req) = extract_csrf_token(&method, &headers, req).await;

                let validation_result = if let Some(token) = provided_token {
                    if !token.is_empty() {
                        validate_csrf_token(&session, &token)
                    } else {
                        Err(bad_request(
                            "CSRF token missing. Please include a CSRF token in your request.",
                        ))
                    }
                } else {
                    Err(bad_request(
                        "CSRF token missing. Please include a CSRF token in your request.",
                    ))
                };

                // Handle validation errors
                if let Err(err) = validation_result {
                    return err.response();
                }

                return next.run(req).await;
            }
        }
    }

    next.run(req).await
}

/// CSRF-only protection middleware
///
/// Validates CSRF tokens for unsafe HTTP methods (POST, PUT, PATCH, DELETE).
/// This middleware does NOT check authentication - it only validates CSRF tokens.
/// Use this for routes that require CSRF protection but may be accessed by anonymous users.
///
/// Safe methods (GET, HEAD, OPTIONS) are allowed without CSRF validation.
///
/// This middleware checks for the CSRF token in:
/// 1. The `X-CSRF-Token` header (for HTMX and API clients)
/// 2. The `csrf_token` form field (for traditional form submissions)
///
/// # Note
///
/// If no session exists or the session is empty, the request passes through unchanged.
pub async fn csrf_protect(req: axum::extract::Request, next: Next) -> Response {
    let method = req.method().clone();
    let headers = req.headers().clone();

    // API token-authenticated requests do not require CSRF protection
    if is_unsafe_method(&method)
        && req
            .extensions()
            .get::<Authentication>()
            .is_some_and(|auth| auth.is_token())
    {
        return next.run(req).await;
    }

    // Only validate unsafe methods if session exists and has CSRF token
    if is_unsafe_method(&method) {
        if let Some(session) = req.extensions().get::<SessionExtension>().cloned() {
            // Only validate if session has CSRF token
            if session.get(CSRF_TOKEN_KEY).is_some() {
                // Extract CSRF token from header or form body
                let (provided_token, req) = extract_csrf_token(&method, &headers, req).await;

                let validation_result = if let Some(token) = provided_token {
                    if !token.is_empty() {
                        validate_csrf_token(&session, &token)
                    } else {
                        Err(bad_request(
                            "CSRF token missing. Please include a CSRF token in your request.",
                        ))
                    }
                } else {
                    Err(bad_request(
                        "CSRF token missing. Please include a CSRF token in your request.",
                    ))
                };

                // Handle validation errors
                if let Err(err) = validation_result {
                    return err.response();
                }

                return next.run(req).await;
            }
        }
    }

    next.run(req).await
}

/// Middleware that ensures a CSRF token exists in the session
///
/// This middleware should be applied to routes that render forms.
/// It ensures that a CSRF token is available in the session before the handler runs.
/// If no session exists, the request passes through unchanged.
pub async fn ensure_token(req: axum::extract::Request, next: Next) -> Response {
    // Only create CSRF token if session exists
    if let Some(session) = req.extensions().get::<SessionExtension>() {
        get_or_create_csrf_token(session);
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length() {
        let token = generate_token();
        assert_eq!(token.len(), 32);
    }

    #[test]
    fn test_generate_token_uniqueness() {
        let token1 = generate_token();
        let token2 = generate_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_generate_token_alphanumeric() {
        let token = generate_token();
        assert!(token.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_is_unsafe_method() {
        assert!(is_unsafe_method(&Method::POST));
        assert!(is_unsafe_method(&Method::PUT));
        assert!(is_unsafe_method(&Method::PATCH));
        assert!(is_unsafe_method(&Method::DELETE));
        assert!(!is_unsafe_method(&Method::GET));
        assert!(!is_unsafe_method(&Method::HEAD));
        assert!(!is_unsafe_method(&Method::OPTIONS));
    }

    #[test]
    fn test_extract_csrf_from_form_data() {
        let form_data = "username=test&csrf_token=abc123&other=value";
        let token = extract_csrf_from_form_data(form_data);
        assert_eq!(token, Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_csrf_from_form_data_none() {
        let form_data = "username=test&other=value";
        let token = extract_csrf_from_form_data(form_data);
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_csrf_from_form_data_empty() {
        let token = extract_csrf_from_form_data("");
        assert!(token.is_none());
    }

    #[tokio::test]
    async fn test_extract_csrf_token_from_header() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(CSRF_HEADER_NAME, "test_token_123".parse().unwrap());

        let req = axum::extract::Request::builder()
            .method(Method::POST)
            .body(Body::empty())
            .unwrap();

        let (token, _) = extract_csrf_token(&Method::POST, &headers, req).await;
        assert_eq!(token, Some("test_token_123".to_string()));
    }

    #[tokio::test]
    async fn test_extract_csrf_token_safe_method() {
        let req = axum::extract::Request::builder()
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let (token, _) = extract_csrf_token(&Method::GET, &axum::http::HeaderMap::new(), req).await;
        assert_eq!(token, Some(String::new()));
    }

    #[tokio::test]
    async fn test_extract_csrf_token_from_form_body() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );

        let body = "username=test&csrf_token=abc123&other=value";
        let req = axum::extract::Request::builder()
            .method(Method::POST)
            .header(
                axum::http::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(Body::from(body))
            .unwrap();

        let (token, _) = extract_csrf_token(&Method::POST, &headers, req).await;
        assert_eq!(token, Some("abc123".to_string()));
    }

    #[tokio::test]
    async fn test_extract_csrf_token_none() {
        let req = axum::extract::Request::builder()
            .method(Method::POST)
            .body(Body::empty())
            .unwrap();

        let (token, _) =
            extract_csrf_token(&Method::POST, &axum::http::HeaderMap::new(), req).await;
        assert!(token.is_none());
    }
}

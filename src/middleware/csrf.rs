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
//! ```rust
//! use axum::{Form, extract::Extension};
//! use crate::middleware::{SessionExtension, CsrfToken};
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

use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use rand::distributions::Alphanumeric;
use rand::Rng;

use crate::middleware::SessionExtension;
use crate::util::errors::{bad_request, AppResult};

pub static CSRF_TOKEN_KEY: &str = "csrf_token";
pub static CSRF_HEADER_NAME: &str = "x-csrf-token";
pub static CSRF_FORM_FIELD: &str = "csrf_token";

/// Generate a cryptographically secure random CSRF token
pub fn generate_token() -> String {
    rand::thread_rng()
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

    if session_token == provided_token {
        Ok(())
    } else {
        Err(bad_request(
            "Invalid CSRF token. Please refresh the page and try again.",
        ))
    }
}

/// Extract CSRF token from request (header or form field)
fn extract_csrf_token_from_request(
    method: &Method,
    headers: &axum::http::HeaderMap,
    form_data: Option<&str>,
) -> Option<String> {
    // Only validate unsafe methods
    if !is_unsafe_method(method) {
        return Some(String::new()); // Safe methods don't need CSRF
    }

    // First check header (for HTMX and API clients)
    if let Some(header_value) = headers.get(CSRF_HEADER_NAME) {
        if let Ok(token) = header_value.to_str() {
            return Some(token.to_string());
        }
    }

    // Then check form field
    if let Some(form_data) = form_data {
        for pair in form_data.split('&') {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() == 2 && parts[0] == CSRF_FORM_FIELD {
                if let Ok(decoded) = urlencoding::decode(parts[1]) {
                    return Some(decoded.into_owned());
                }
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
    let method = req.method();
    let headers = req.headers();

    // Only validate unsafe methods if session exists and has data
    if is_unsafe_method(method) {
        if let Some(session) = req.extensions().get::<SessionExtension>() {
            // Only validate if session has actual data (not empty/anonymous)
            if session.get("user_id").is_some() || session.get(CSRF_TOKEN_KEY).is_some() {
                // Try to extract CSRF token from header
                let provided_token = extract_csrf_token_from_request(method, headers, None);

                let validation_result = if let Some(token) = provided_token {
                    if !token.is_empty() {
                        validate_csrf_token(session, &token)
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
    fn test_extract_csrf_token_from_header() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(CSRF_HEADER_NAME, "test_token_123".parse().unwrap());

        let token = extract_csrf_token_from_request(&Method::POST, &headers, None);
        assert_eq!(token, Some("test_token_123".to_string()));
    }

    #[test]
    fn test_extract_csrf_token_from_form() {
        let form_data = "username=test&csrf_token=abc123&other=value";

        let token = extract_csrf_token_from_request(
            &Method::POST,
            &axum::http::HeaderMap::new(),
            Some(form_data),
        );
        assert_eq!(token, Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_csrf_token_safe_method() {
        let token =
            extract_csrf_token_from_request(&Method::GET, &axum::http::HeaderMap::new(), None);
        // Safe methods return empty string (valid)
        assert_eq!(token, Some(String::new()));
    }

    #[test]
    fn test_extract_csrf_token_none() {
        let token =
            extract_csrf_token_from_request(&Method::POST, &axum::http::HeaderMap::new(), None);
        assert!(token.is_none());
    }
}

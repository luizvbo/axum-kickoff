//! Error handling utilities
//!
//! This module implements error types and traits for consistent error handling
//! across the application, following the pattern from crates.io.
//!
//! # Usage
//!
//! - Use `AppError` trait for errors that should be converted to HTTP responses
//! - Use `AppResult<T>` as a shorthand for `Result<T, Box<dyn AppError>>`
//! - Use helper functions like `bad_request()`, `forbidden()`, `not_found()` for common errors
//! - Use `util::Error` (from thiserror) for non-HTTP errors

use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use serde::Serialize;
use std::any::TypeId;
use std::borrow::Cow;
use std::fmt;

/// Type alias for results that can be converted to HTTP responses
pub type AppResult<T> = Result<T, Box<dyn AppError>>;

/// Trait for errors that can be converted to HTTP responses
///
/// This trait should be implemented for domain-specific errors that need
/// to be returned to the client as JSON responses.
pub trait AppError: Send + fmt::Display + fmt::Debug + 'static {
    /// Generate an HTTP response for the error
    ///
    /// If `None` is returned, the error will bubble up the middleware stack
    /// where it is eventually logged and turned into a status 500 response.
    fn response(&self) -> Response;

    /// Get the TypeId of the error
    fn get_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

/// JSON error response structure
#[derive(Serialize)]
struct ErrorResponse {
    detail: String,
}

/// Return an error with status 400 and the provided description as JSON
pub fn bad_request(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::BAD_REQUEST, detail))
}

/// Return an error with status 403 and the provided description as JSON
pub fn forbidden(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::FORBIDDEN, detail))
}

/// Return an error with status 404
pub fn not_found() -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::NOT_FOUND, "Not Found"))
}

/// Return an error with status 401
pub fn unauthorized(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::UNAUTHORIZED, detail))
}

/// Return an error with status 500 and the provided description as JSON
pub fn server_error(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, detail))
}

/// Return an error with status 503
pub fn service_unavailable() -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::SERVICE_UNAVAILABLE, "Service unavailable"))
}

/// Generic HTTP error with a status code and detail message
#[derive(Debug)]
struct HttpError {
    status: StatusCode,
    detail: Cow<'static, str>,
}

impl HttpError {
    fn new(status: StatusCode, detail: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.status, self.detail)
    }
}

impl AppError for HttpError {
    fn response(&self) -> Response {
        let error_response = ErrorResponse {
            detail: self.detail.to_string(),
        };
        (self.status, Json(error_response)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_request_error() {
        let error = bad_request("Invalid input");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_forbidden_error() {
        let error = forbidden("Access denied");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_not_found_error() {
        let error = not_found();
        let response = error.response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_unauthorized_error() {
        let error = unauthorized("Invalid token");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_server_error() {
        let error = server_error("Database connection failed");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_service_unavailable_error() {
        let error = service_unavailable();
        let response = error.response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

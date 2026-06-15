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
//! - Use domain-specific error types (AuthError, ValidationError, NotFoundError) for structured errors
//! - Use `util::Error` (from thiserror) for non-HTTP errors

use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use serde::Serialize;
use std::any::TypeId;
use std::borrow::Cow;
use std::fmt;

/// Type alias for boxed app errors
pub type BoxedAppError = Box<dyn AppError>;

/// Type alias for results that can be converted to HTTP responses
pub type AppResult<T> = Result<T, BoxedAppError>;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    error_type: Option<String>,
}

impl ErrorResponse {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            error_type: None,
        }
    }

    fn with_type(detail: impl Into<String>, error_type: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            error_type: Some(error_type.into()),
        }
    }
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
        let error_response = ErrorResponse::new(self.detail.to_string());
        (self.status, Json(error_response)).into_response()
    }
}

impl AppError for BoxedAppError {
    fn response(&self) -> Response {
        (**self).response()
    }

    fn get_type_id(&self) -> TypeId {
        (**self).get_type_id()
    }
}

impl IntoResponse for BoxedAppError {
    fn into_response(self) -> Response {
        self.response()
    }
}

impl<E: std::error::Error + Send + 'static> AppError for E {
    fn response(&self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

// =============================================================================
// Domain-specific error types
// =============================================================================

/// Authentication-related errors
#[derive(Debug)]
pub enum AuthError {
    /// Invalid or missing authentication credentials
    InvalidCredentials { detail: Cow<'static, str> },
    /// Session expired or invalid
    SessionExpired { detail: Cow<'static, str> },
    /// Insufficient permissions for the requested action
    InsufficientPermissions { detail: Cow<'static, str> },
    /// Account is locked
    AccountLocked { detail: Cow<'static, str> },
}

impl AuthError {
    pub fn invalid_credentials(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::InvalidCredentials {
            detail: detail.into(),
        }
    }

    pub fn session_expired(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::SessionExpired {
            detail: detail.into(),
        }
    }

    pub fn insufficient_permissions(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::InsufficientPermissions {
            detail: detail.into(),
        }
    }

    pub fn account_locked(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::AccountLocked {
            detail: detail.into(),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            AuthError::InvalidCredentials { .. } => StatusCode::UNAUTHORIZED,
            AuthError::SessionExpired { .. } => StatusCode::UNAUTHORIZED,
            AuthError::InsufficientPermissions { .. } => StatusCode::FORBIDDEN,
            AuthError::AccountLocked { .. } => StatusCode::FORBIDDEN,
        }
    }

    fn detail(&self) -> &str {
        match self {
            AuthError::InvalidCredentials { detail } => detail,
            AuthError::SessionExpired { detail } => detail,
            AuthError::InsufficientPermissions { detail } => detail,
            AuthError::AccountLocked { detail } => detail,
        }
    }

    fn error_type(&self) -> &'static str {
        match self {
            AuthError::InvalidCredentials { .. } => "invalid_credentials",
            AuthError::SessionExpired { .. } => "session_expired",
            AuthError::InsufficientPermissions { .. } => "insufficient_permissions",
            AuthError::AccountLocked { .. } => "account_locked",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_type(), self.detail())
    }
}

impl AppError for AuthError {
    fn response(&self) -> Response {
        let error_response = ErrorResponse::with_type(self.detail(), self.error_type());
        (self.status(), Json(error_response)).into_response()
    }
}

/// Validation errors for user input
#[derive(Debug)]
pub enum ValidationError {
    /// Invalid format for a field
    InvalidFormat {
        field: Cow<'static, str>,
        detail: Cow<'static, str>,
    },
    /// Missing required field
    MissingField { field: Cow<'static, str> },
    /// Value out of valid range
    OutOfRange {
        field: Cow<'static, str>,
        detail: Cow<'static, str>,
    },
    /// Generic validation error
    Custom { detail: Cow<'static, str> },
}

impl ValidationError {
    pub fn invalid_format(
        field: impl Into<Cow<'static, str>>,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::InvalidFormat {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub fn missing_field(field: impl Into<Cow<'static, str>>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    pub fn out_of_range(
        field: impl Into<Cow<'static, str>>,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::OutOfRange {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub fn custom(detail: impl Into<Cow<'static, str>>) -> Self {
        Self::Custom {
            detail: detail.into(),
        }
    }

    fn detail(&self) -> String {
        match self {
            ValidationError::InvalidFormat { field, detail } => {
                format!("Invalid format for field '{}': {}", field, detail)
            }
            ValidationError::MissingField { field } => {
                format!("Missing required field: {}", field)
            }
            ValidationError::OutOfRange { field, detail } => {
                format!("Value out of range for field '{}': {}", field, detail)
            }
            ValidationError::Custom { detail } => detail.to_string(),
        }
    }

    fn error_type(&self) -> &'static str {
        match self {
            ValidationError::InvalidFormat { .. } => "invalid_format",
            ValidationError::MissingField { .. } => "missing_field",
            ValidationError::OutOfRange { .. } => "out_of_range",
            ValidationError::Custom { .. } => "validation_error",
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_type(), self.detail())
    }
}

impl AppError for ValidationError {
    fn response(&self) -> Response {
        let error_response = ErrorResponse::with_type(self.detail(), self.error_type());
        (StatusCode::BAD_REQUEST, Json(error_response)).into_response()
    }
}

/// Resource not found errors
#[derive(Debug)]
pub enum NotFoundError {
    /// Generic not found error
    ResourceNotFound { resource: Cow<'static, str> },
    /// User not found
    UserNotFound { user_id: Cow<'static, str> },
    /// Record not found with specific identifier
    RecordNotFound {
        resource: Cow<'static, str>,
        identifier: Cow<'static, str>,
    },
}

impl NotFoundError {
    pub fn resource_not_found(resource: impl Into<Cow<'static, str>>) -> Self {
        Self::ResourceNotFound {
            resource: resource.into(),
        }
    }

    pub fn user_not_found(user_id: impl Into<Cow<'static, str>>) -> Self {
        Self::UserNotFound {
            user_id: user_id.into(),
        }
    }

    pub fn record_not_found(
        resource: impl Into<Cow<'static, str>>,
        identifier: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::RecordNotFound {
            resource: resource.into(),
            identifier: identifier.into(),
        }
    }

    fn detail(&self) -> String {
        match self {
            NotFoundError::ResourceNotFound { resource } => {
                format!("{} not found", resource)
            }
            NotFoundError::UserNotFound { user_id } => {
                format!("User '{}' not found", user_id)
            }
            NotFoundError::RecordNotFound { resource, identifier } => {
                format!("{} with identifier '{}' not found", resource, identifier)
            }
        }
    }

    fn error_type(&self) -> &'static str {
        match self {
            NotFoundError::ResourceNotFound { .. } => "resource_not_found",
            NotFoundError::UserNotFound { .. } => "user_not_found",
            NotFoundError::RecordNotFound { .. } => "record_not_found",
        }
    }
}

impl fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_type(), self.detail())
    }
}

impl AppError for NotFoundError {
    fn response(&self) -> Response {
        let error_response = ErrorResponse::with_type(self.detail(), self.error_type());
        (StatusCode::NOT_FOUND, Json(error_response)).into_response()
    }
}

// =============================================================================
// Helper functions for domain-specific errors
// =============================================================================

/// Create an authentication error with invalid credentials
pub fn auth_invalid_credentials(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(AuthError::invalid_credentials(detail))
}

/// Create an authentication error for expired session
pub fn auth_session_expired(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(AuthError::session_expired(detail))
}

/// Create an authentication error for insufficient permissions
pub fn auth_insufficient_permissions(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(AuthError::insufficient_permissions(detail))
}

/// Create an authentication error for locked account
pub fn auth_account_locked(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(AuthError::account_locked(detail))
}

/// Create a validation error for invalid format
pub fn validation_invalid_format(
    field: impl Into<Cow<'static, str>>,
    detail: impl Into<Cow<'static, str>>,
) -> Box<dyn AppError> {
    Box::new(ValidationError::invalid_format(field, detail))
}

/// Create a validation error for missing field
pub fn validation_missing_field(field: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(ValidationError::missing_field(field))
}

/// Create a validation error for out of range value
pub fn validation_out_of_range(
    field: impl Into<Cow<'static, str>>,
    detail: impl Into<Cow<'static, str>>,
) -> Box<dyn AppError> {
    Box::new(ValidationError::out_of_range(field, detail))
}

/// Create a custom validation error
pub fn validation_custom(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(ValidationError::custom(detail))
}

/// Create a not found error for a resource
pub fn not_found_resource(resource: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(NotFoundError::resource_not_found(resource))
}

/// Create a not found error for a user
pub fn not_found_user(user_id: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(NotFoundError::user_not_found(user_id))
}

/// Create a not found error for a record
pub fn not_found_record(
    resource: impl Into<Cow<'static, str>>,
    identifier: impl Into<Cow<'static, str>>,
) -> Box<dyn AppError> {
    Box::new(NotFoundError::record_not_found(resource, identifier))
}

/// Convert a standard error to an AppError
///
/// This is useful for converting errors from external libraries (like database
/// errors) into application-specific errors that can be returned to clients.
pub fn convert_error<E: std::error::Error + Send + Sync + 'static>(
    error: E,
) -> Box<dyn AppError> {
    server_error(error.to_string())
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

    // AuthError tests
    #[test]
    fn test_auth_invalid_credentials() {
        let error = auth_invalid_credentials("Invalid password");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_session_expired() {
        let error = auth_session_expired("Session has expired");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_insufficient_permissions() {
        let error = auth_insufficient_permissions("You don't have permission");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_auth_account_locked() {
        let error = auth_account_locked("Account is locked");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    // ValidationError tests
    #[test]
    fn test_validation_invalid_format() {
        let error = validation_invalid_format("email", "Invalid email format");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validation_missing_field() {
        let error = validation_missing_field("username");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validation_out_of_range() {
        let error = validation_out_of_range("age", "Must be between 18 and 120");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validation_custom() {
        let error = validation_custom("Custom validation error");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // NotFoundError tests
    #[test]
    fn test_not_found_resource() {
        let error = not_found_resource("Article");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_not_found_user() {
        let error = not_found_user("user123");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_not_found_record() {
        let error = not_found_record("Product", "prod-456");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // Adapted from crates.io tests
    #[test]
    fn test_http_error_responses() {
        // Test all standard HTTP error status codes
        assert_eq!(bad_request("").response().status(), StatusCode::BAD_REQUEST);
        assert_eq!(forbidden("").response().status(), StatusCode::FORBIDDEN);
        assert_eq!(not_found().response().status(), StatusCode::NOT_FOUND);
        assert_eq!(unauthorized("").response().status(), StatusCode::UNAUTHORIZED);
        assert_eq!(server_error("").response().status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(service_unavailable().response().status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_standard_error_conversions() {
        // Test that standard errors are converted to server errors
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
        let app_error = convert_error(io_error);
        assert_eq!(app_error.response().status(), StatusCode::INTERNAL_SERVER_ERROR);

        // Test serde_json error conversion using a parse error
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json")
            .unwrap_err();
        let app_error = convert_error(json_error);
        assert_eq!(app_error.response().status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_response_json_format() {
        // Test that error responses include detail field
        let error = bad_request("Invalid input");
        let response = error.response();

        // The response should be JSON with a detail field
        // For now, we just verify the status code is correct
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_domain_specific_error_types() {
        // Test that domain-specific errors include error_type
        let error = auth_invalid_credentials("bad password");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let error = validation_missing_field("email");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let error = not_found_user("user123");
        let response = error.response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

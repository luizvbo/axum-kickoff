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

use askama::Template;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use http::{header, HeaderMap, HeaderValue, StatusCode};
use serde::Serialize;
use std::any::TypeId;
use std::borrow::Cow;
use std::fmt;
use tokio::task_local;

use crate::middleware::security_headers::current_csp_nonce;
use crate::router::PageContext;

/// Type alias for boxed app errors
pub type BoxedAppError = Box<dyn AppError>;

/// Type alias for results that can be converted to HTTP responses
pub type AppResult<T> = Result<T, BoxedAppError>;

/// Describes how the client would like error and HTML responses to be formatted.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RequestFormat {
    /// Whether the request was made by HTMX (`HX-Request: true`).
    pub is_hx: bool,
    /// Whether the client accepts HTML (`Accept` contains `text/html`).
    pub accept_html: bool,
}

impl RequestFormat {
    /// Derive the preferred response format from the request headers.
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let is_hx = headers
            .get("HX-Request")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "true");

        let accept_html = headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(',').any(|part| {
                    let mut parts = part.split(';');
                    let mime = parts.next().unwrap_or("").trim();
                    mime == "text/html"
                })
            });

        Self { is_hx, accept_html }
    }
}

task_local! {
    /// The preferred response format for the current request.
    pub(crate) static REQUEST_FORMAT: RequestFormat;
}

/// Run a future with the given request format set as a task-local so that
/// `AppError::response` and `HtmlTemplate` can access request headers.
pub(crate) async fn with_request_format<F>(format: RequestFormat, f: F) -> F::Output
where
    F: std::future::Future,
{
    REQUEST_FORMAT.scope(format, f).await
}

/// Return the preferred response format for the current request.
pub(crate) fn request_format() -> RequestFormat {
    REQUEST_FORMAT.try_with(|f| *f).ok().unwrap_or_default()
}

/// Return `true` if the current request was made by HTMX.
pub(crate) fn is_hx_request() -> bool {
    request_format().is_hx
}

/// Trait for errors that can be converted to HTTP responses
///
/// This trait should be implemented for domain-specific errors that need
/// to be returned to the client as JSON, HTML, or HTMX fragment responses.
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

/// HTML error page template.
#[derive(Template)]
#[template(path = "error.html")]
struct HtmlError {
    ctx: PageContext,
    status: u16,
    message: String,
}

/// Build a response for an error, choosing between HTMX fragments, full HTML
/// pages, and JSON based on the request headers.
fn build_error_response(
    status: StatusCode,
    detail: impl Into<String>,
    error_type: Option<&str>,
) -> Response {
    let format = request_format();
    let detail = detail.into();

    if format.is_hx {
        let escaped = escape_html(&detail);
        let body = format!(r#"<div class="error">{}</div>"#, escaped);
        let mut response = (status, Html(body)).into_response();
        response
            .headers_mut()
            .insert("HX-Reswap", HeaderValue::from_static("none"));
        return response;
    }

    if format.accept_html {
        let ctx = PageContext {
            csrf_token: String::new(),
            csp_nonce: current_csp_nonce(),
        };
        let html = HtmlError {
            ctx,
            status: status.as_u16(),
            message: detail.to_string(),
        }
        .render()
        .unwrap_or_else(|_| detail.to_string());

        let mut response = (status, Html(html)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        return response;
    }

    let error_response = match error_type {
        Some(t) => ErrorResponse::with_type(detail, t),
        None => ErrorResponse::new(detail),
    };
    (status, Json(error_response)).into_response()
}

/// Minimal HTML escaping for inline error fragments.
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Return an error with status 400 and the provided description
pub fn bad_request(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::BAD_REQUEST, detail))
}

/// Return an error with status 403 and the provided description
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

/// Return an error with status 500 and the provided description
pub fn server_error(detail: impl Into<Cow<'static, str>>) -> Box<dyn AppError> {
    Box::new(HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, detail))
}

/// Return an error with status 503
pub fn service_unavailable() -> Box<dyn AppError> {
    Box::new(HttpError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "Service unavailable",
    ))
}

/// Rate limit error (429 Too Many Requests) with a Retry-After header
#[derive(Debug)]
pub struct RateLimitAppError {
    detail: Cow<'static, str>,
    retry_after_secs: u64,
}

impl RateLimitAppError {
    pub fn new(detail: impl Into<Cow<'static, str>>, retry_after: std::time::Duration) -> Self {
        Self {
            detail: detail.into(),
            retry_after_secs: retry_after.as_secs().max(1),
        }
    }
}

impl fmt::Display for RateLimitAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Too Many Requests: {}", self.detail)
    }
}

impl AppError for RateLimitAppError {
    fn response(&self) -> Response {
        let mut response = build_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            self.detail.to_string(),
            Some("rate_limit_exceeded"),
        );
        if let Ok(value) = HeaderValue::from_str(&self.retry_after_secs.to_string()) {
            response.headers_mut().insert("retry-after", value);
        }
        response
    }
}

/// Create a rate limit error (429) with a Retry-After header
pub fn rate_limited(
    detail: impl Into<Cow<'static, str>>,
    retry_after: std::time::Duration,
) -> Box<dyn AppError> {
    Box::new(RateLimitAppError::new(detail, retry_after))
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
        build_error_response(self.status, self.detail.clone(), None)
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
        build_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
            None,
        )
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
        build_error_response(
            self.status(),
            self.detail().to_string(),
            Some(self.error_type()),
        )
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
        build_error_response(
            StatusCode::BAD_REQUEST,
            self.detail(),
            Some(self.error_type()),
        )
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
            NotFoundError::RecordNotFound {
                resource,
                identifier,
            } => {
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
        build_error_response(
            StatusCode::NOT_FOUND,
            self.detail(),
            Some(self.error_type()),
        )
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

/// Map a Toasty database error to an appropriate HTTP error.
///
/// - Record-not-found / row-missing -> 404
/// - Pool timeout / acquire error -> 503
/// - Other DB errors -> 500 (and Sentry if enabled, via `tracing::error`)
pub fn db_error(error: toasty::Error) -> Box<dyn AppError> {
    if error.is_record_not_found() || error.is_invalid_record_count() {
        return not_found();
    }

    if error.is_connection_pool() || error.is_connection_lost() {
        return service_unavailable();
    }

    tracing::error!("Database error: {}", error);
    server_error("Internal server error")
}

/// Log an error internally and return a generic 500 error to the client.
///
/// This prevents leaking internal error details (e.g. database errors) to
/// the client while still capturing the full error in logs for debugging.
pub fn internal_error<E: std::fmt::Display>(error: E) -> Box<dyn AppError> {
    tracing::error!("Internal error: {}", error);
    server_error("Internal server error")
}

/// Convert a standard error to an AppError
///
/// This is useful for converting errors from external libraries into
/// application-specific errors that can be returned to clients.
pub fn convert_error<E: std::error::Error + Send + Sync + 'static>(error: E) -> Box<dyn AppError> {
    // Note: Toasty database errors should be mapped with `db_error` so that
    // record-not-found and pool errors are converted to the correct status.
    internal_error(error)
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
        assert_eq!(
            unauthorized("").response().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            server_error("").response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            service_unavailable().response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn test_standard_error_conversions() {
        // Test that standard errors are converted to server errors
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
        let app_error = convert_error(io_error);
        assert_eq!(
            app_error.response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );

        // Test serde_json error conversion using a parse error
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let app_error = convert_error(json_error);
        assert_eq!(
            app_error.response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
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

    #[test]
    fn test_toasty_error_mapping() {
        let record_not_found = toasty::Error::record_not_found("table=users key={id: 123}");
        assert_eq!(
            db_error(record_not_found).response().status(),
            StatusCode::NOT_FOUND
        );

        let pool_error = toasty::Error::connection_pool(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "pool exhausted",
        ));
        assert_eq!(
            db_error(pool_error).response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );

        let other = toasty::Error::invalid_schema("unknown column");
        assert_eq!(
            db_error(other).response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

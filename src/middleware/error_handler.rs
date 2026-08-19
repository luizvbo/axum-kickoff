//! Error handling middleware
//!
//! This middleware provides consistent error handling across the application.
//! It works with Axum's built-in error handling by catching errors that implement
//! the `AppError` trait and converting them to appropriate HTTP responses.
//!
//! # Usage
//!
//! The middleware is automatically applied in the middleware stack. Handlers should
//! return `AppResult<T>` (which is `Result<T, Box<dyn AppError>>`) for errors that
//! should be converted to HTTP responses.
//!
//! # Error Response Format
//!
//! Errors return JSON, HTML, or HTMX fragment responses depending on the request
//! headers (`Accept` and `HX-Request`).
//!
//! # Example
//!
//! The `not_found_user` function can be used to create a "user not found" error:
//!
//! ```text
//! not_found_user("user_123")
//! ```

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing::{error, warn};

use crate::util::errors::{with_request_format, RequestFormat};

/// Error handling middleware
///
/// This middleware ensures that errors implementing `AppError` are properly
/// converted to HTTP responses. Axum's built-in error handling will call the
/// `response()` method on `AppError` implementations automatically.
///
/// This middleware primarily serves as a logging layer for errors that
/// weren’t handled by the application's error handling logic.
pub async fn middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let format = RequestFormat::from_headers(req.headers());
    let response = with_request_format(format, async {
        let response = next.run(req).await;

        // Log error responses for monitoring
        if response.status().is_server_error() {
            error!(
                "Server error response: {} {} - Status: {}",
                method,
                uri,
                response.status()
            );
        } else if response.status().is_client_error() {
            warn!(
                "Client error response: {} {} - Status: {}",
                method,
                uri,
                response.status()
            );
        }

        response
    })
    .await;

    response
}

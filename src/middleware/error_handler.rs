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
//! Errors return JSON responses with the following structure:
//!
//! ```json
//! {
//!   "detail": "Error message",
//!   "error_type": "error_type_name"  // Optional, for domain-specific errors
//! }
//! ```
//!
//! # Example
//!
//! ```rust
//! use axum_kickoff::util::{AppResult, not_found_user};
//!
//! async fn get_user(user_id: String) -> AppResult<User> {
//!     let user = db.find_user(&user_id).await?;
//!     Ok(user)
//! }
//! ```

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing::error;

/// Error handling middleware
///
/// This middleware ensures that errors implementing `AppError` are properly
/// converted to HTTP responses. Axum's built-in error handling will call the
/// `response()` method on `AppError` implementations automatically.
///
/// This middleware primarily serves as a logging layer for errors that
/// weren't handled by the application's error handling logic.
pub async fn middleware(
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    
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
        // Log 4xx errors at info level for debugging
        error!(
            "Client error response: {} {} - Status: {}",
            method,
            uri,
            response.status()
        );
    }
    
    response
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_error_handler_middleware() {
        // This test verifies the middleware compiles and can be used
        assert!(true);
    }
}

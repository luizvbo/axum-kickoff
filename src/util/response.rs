//! Consistent API response wrappers
//!
//! Provides standardized response structures for API endpoints.
//! Note: Error responses use the simpler format in src/util/errors.rs:
//! { "detail": "...", "error_type": "..." } rather than the
//! crates.io-style { "errors": [{ "detail": "..." }] } format.

use serde::Serialize;

/// Standard success response wrapper
///
/// Wraps successful responses in a consistent structure with a data field.
/// This matches the crates.io pattern for API responses.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a new API response
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

// Note: Error responses are handled by src/util/errors.rs which uses a simpler
// { "detail": "...", "error_type": "..." } format instead of the crates.io-style
// { "errors": [{ "detail": "..." }] } format. This was intentionally simplified
// for better ergonomics in a generic template.

/// Helper to wrap data in an API response
pub fn response<T: Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse::new(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response() {
        let data = vec!["item1", "item2"];
        let response = ApiResponse::new(data.clone());
        assert_eq!(response.data, data);
    }
}

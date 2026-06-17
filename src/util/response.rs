//! Consistent API response wrappers
//!
//! Provides standardized response structures for API endpoints following
//! the pattern from crates.io with consistent shapes for success and error responses.

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

/// Standard error response wrapper
///
/// Wraps error responses in a consistent structure with an errors array.
/// This matches the crates.io pattern for error responses.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub errors: Vec<ErrorDetail>,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub detail: String,
}

impl ErrorResponse {
    /// Create a new error response from a single error message
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            errors: vec![ErrorDetail {
                detail: detail.into(),
            }],
        }
    }

    /// Create a new error response from multiple error messages
    pub fn from_details(details: Vec<String>) -> Self {
        Self {
            errors: details
                .into_iter()
                .map(|detail| ErrorDetail { detail })
                .collect(),
        }
    }
}

/// Helper to wrap data in an API response
pub fn response<T: Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse::new(data)
}

/// Helper to create an error response
pub fn error_response(detail: impl Into<String>) -> ErrorResponse {
    ErrorResponse::new(detail)
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

    #[test]
    fn test_error_response() {
        let response = ErrorResponse::new("Test error");
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].detail, "Test error");
    }

    #[test]
    fn test_error_response_multiple() {
        let details = vec!["Error 1".to_string(), "Error 2".to_string()];
        let response = ErrorResponse::from_details(details.clone());
        assert_eq!(response.errors.len(), 2);
        assert_eq!(response.errors[0].detail, "Error 1");
        assert_eq!(response.errors[1].detail, "Error 2");
    }
}

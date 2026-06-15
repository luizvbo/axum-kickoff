//! Response wrapper for test assertions
//!
//! Adapted from crates.io's test infrastructure to provide ergonomic
//! response handling with status code checking and JSON deserialization.

use axum::http::StatusCode;
use axum::response::Response as AxumResponse;
use http::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use std::fmt;

/// Wrapper around axum responses with helper methods for assertions
pub struct Response<T> {
    inner: AxumResponse,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Response<T> {
    /// Create a new Response from an axum Response
    pub fn new(inner: AxumResponse) -> Self {
        Self {
            inner,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the status code of the response
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Assert that the response has the given status code
    pub fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(
            self.status(),
            expected,
            "Expected status {}, got {}",
            expected,
            self.status()
        );
        self
    }

    /// Assert that the response is successful (2xx)
    pub fn assert_success(&self) -> &Self {
        let status = self.status();
        assert!(
            status.is_success(),
            "Expected successful status (2xx), got {}",
            status
        );
        self
    }

    /// Get the response body as bytes
    pub async fn into_bytes(self) -> Vec<u8> {
        use http_body_util::BodyExt;
        
        let body = self.inner.into_body();
        let bytes = body
            .collect()
            .await
            .expect("Failed to collect response body")
            .to_bytes();
        bytes.to_vec()
    }

    /// Get the response body as a string
    pub async fn into_string(self) -> String {
        let bytes = self.into_bytes().await;
        String::from_utf8(bytes).expect("Response body was not valid UTF-8")
    }

    /// Deserialize the response body as JSON
    pub async fn into_json<U>(self) -> U
    where
        U: DeserializeOwned,
    {
        let bytes = self.into_bytes().await;
        serde_json::from_slice(&bytes).expect("Failed to deserialize response as JSON")
    }

    /// Get a reference to the inner axum Response
    pub fn inner(&self) -> &AxumResponse {
        &self.inner
    }

    /// Get the response headers
    pub fn headers(&self) -> &http::HeaderMap {
        self.inner.headers()
    }

    /// Get the content-type header if present
    pub fn content_type(&self) -> Option<&str> {
        self.inner
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
    }
}

impl<T> fmt::Debug for Response<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status())
            .field("content_type", &self.content_type())
            .finish()
    }
}

/// Response for JSON data
pub type JsonResponse<T> = Response<T>;

/// Response for plain text
pub type TextResponse = Response<()>;

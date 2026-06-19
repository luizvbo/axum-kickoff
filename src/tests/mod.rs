//! Test infrastructure
//!
//! This module provides testing utilities adapted from crates.io's
//! test infrastructure, simplified for axum-kickoff's architecture.
//!
//! # Overview
//!
//! The test infrastructure provides:
//! - `TestApp`: A test-ready application with isolated database
//! - `RequestHelper`: Trait for making authenticated requests
//! - `Response`: Wrapper for response assertions
//! - `AnonymousUser`, `CookieUser`, `TokenUser`: Authentication states
//! - `builders`: Fluent API for creating test data
//!
//! # Usage
//!
//! ```rust
//! use axum_kickoff::tests::{TestApp, AnonymousUser};
//!
//! #[tokio::test]
//! async fn test_example() {
//!     let app = TestApp::new();
//!     let anon = AnonymousUser::new(app);
//!
//!     let response = anon.get::<serde_json::Value>("/health").await;
//!     response.assert_status(http::StatusCode::OK);
//! }
//! ```
//!
//! # Snapshot Testing
//!
//! This crate uses `insta` for snapshot testing. To update snapshots:
//!
//! ```bash
//! cargo insta accept
//! ```

pub mod auth;
pub mod builders;
pub mod error_responses;
pub mod middleware;
pub mod middleware_auth;
pub mod request_helper;
pub mod response;
pub mod test_app;

// Re-export commonly used types for convenience
pub use request_helper::{AnonymousUser, CookieUser, RequestHelper, TokenUser};
pub use response::{JsonResponse, Response, TextResponse};
pub use test_app::TestApp;

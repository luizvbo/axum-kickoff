//! Utility modules

pub mod errors;

pub use errors::{AppError, AppResult, bad_request, forbidden, not_found, unauthorized, server_error, service_unavailable};

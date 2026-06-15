//! Utility modules

pub mod auth;
pub mod errors;
pub mod gh_token_encryption;
pub mod oauth;
pub mod token;

pub use auth::{AuthCheck, AuthHeader, Authentication};
pub use errors::{
    auth_account_locked, auth_insufficient_permissions, auth_invalid_credentials,
    auth_session_expired, bad_request, convert_error, forbidden, not_found, not_found_record,
    not_found_resource, not_found_user, server_error, service_unavailable, unauthorized,
    validation_custom, validation_invalid_format, validation_missing_field,
    validation_out_of_range, AppError, AppResult, AuthError, NotFoundError, ValidationError,
};
pub use gh_token_encryption::GitHubTokenEncryption;
pub use oauth::ReqwestClient;
pub use token::{HashedToken, InvalidTokenError, PlainToken};

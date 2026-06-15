//! Utility modules

pub mod errors;
pub mod oauth;

pub use errors::{
    AppError, AppResult, AuthError, NotFoundError, ValidationError,
    bad_request, forbidden, not_found, unauthorized, server_error, service_unavailable,
    auth_invalid_credentials, auth_session_expired, auth_insufficient_permissions, auth_account_locked,
    validation_invalid_format, validation_missing_field, validation_out_of_range, validation_custom,
    not_found_resource, not_found_user, not_found_record,
    convert_error,
};
pub use oauth::ReqwestClient;

//! API token management controller
//!
//! Provides endpoints for creating, listing, and revoking API tokens.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::util::errors::AppResult;

/// Request body for creating a new API token
#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    /// The name of the token
    pub name: String,
    /// Optional crate scope patterns
    pub crate_scopes: Option<Vec<String>>,
    /// Optional endpoint scopes
    pub endpoint_scopes: Option<Vec<String>>,
    /// Optional expiration date (ISO 8601 format)
    pub expired_at: Option<String>,
}

/// Response for creating a new API token
#[derive(Debug, Serialize)]
pub struct CreateTokenResponse {
    /// The plain text token (only shown once)
    pub token: String,
    /// The token ID
    pub id: u64,
    /// The token name
    pub name: String,
    /// The date and time when the token was created
    pub created_at: String,
    /// Crate scopes
    pub crate_scopes: Option<Vec<String>>,
    /// Endpoint scopes
    pub endpoint_scopes: Option<Vec<String>>,
    /// Expiration date
    pub expired_at: Option<String>,
}

/// Response for listing API tokens
#[derive(Debug, Serialize)]
pub struct TokenListItem {
    /// The token ID
    pub id: u64,
    /// The token name
    pub name: String,
    /// The date and time when the token was created
    pub created_at: String,
    /// The date and time when the token was last used
    pub last_used_at: Option<String>,
    /// Whether the token has been revoked
    pub revoked: bool,
    /// Crate scopes
    pub crate_scopes: Option<Vec<String>>,
    /// Endpoint scopes
    pub endpoint_scopes: Option<Vec<String>>,
    /// Expiration date
    pub expired_at: Option<String>,
}

/// Create a new API token
///
/// This endpoint creates a new API token for the authenticated user.
/// The token is returned in plain text and should be stored securely by the client.
pub async fn create_token(
    State(_state): State<AppState>,
    Json(_req): Json<CreateTokenRequest>,
) -> AppResult<impl IntoResponse> {
    // TODO: Implement full token creation once Toasty proc macro ABI mismatch is resolved
    // The current toolchain (rustc 1.96.0) doesn't match the cached proc macros (rustc 1.94.0)
    // 
    // Full implementation should:
    // 1. Get user_id from session or API token auth
    // 2. Validate crate_scopes and endpoint_scopes
    // 3. Parse expired_at if provided
    // 4. Generate a new PlainToken
    // 5. Hash the token for storage
    // 6. Create ApiToken record in database
    // 7. Return the plain token to the user (only shown once)
    
    // For now, return service unavailable
    Ok((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
        "error": "Token creation is temporarily unavailable due to database layer issues"
    }))))
}

/// List all API tokens for the authenticated user
///
/// This endpoint returns a list of all API tokens belonging to the authenticated user.
pub async fn list_tokens(
    State(_state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    // TODO: Implement full token listing once Toasty proc macro ABI mismatch is resolved
    // 
    // Full implementation should:
    // 1. Get user_id from session or API token auth
    // 2. Query api_tokens table for user's tokens
    // 3. Return list of tokens (without the actual token values)
    
    // For now, return service unavailable
    Ok((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
        "error": "Token listing is temporarily unavailable due to database layer issues"
    }))))
}

/// Revoke an API token
///
/// This endpoint revokes (deletes) an API token by ID.
pub async fn revoke_token(
    State(_state): State<AppState>,
    Path(_token_id): Path<u64>,
) -> AppResult<impl IntoResponse> {
    // TODO: Implement full token revocation once Toasty proc macro ABI mismatch is resolved
    // 
    // Full implementation should:
    // 1. Get user_id from session or API token auth
    // 2. Verify the token belongs to the user
    // 3. Mark the token as revoked in the database
    // 4. Return success
    
    // For now, return service unavailable
    Ok((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
        "error": "Token revocation is temporarily unavailable due to database layer issues"
    }))))
}

/// Get details of a specific API token
///
/// This endpoint returns details of a specific API token by ID.
pub async fn get_token(
    State(_state): State<AppState>,
    Path(_token_id): Path<u64>,
) -> AppResult<impl IntoResponse> {
    // TODO: Implement full token retrieval once Toasty proc macro ABI mismatch is resolved
    // 
    // Full implementation should:
    // 1. Get user_id from session or API token auth
    // 2. Verify the token belongs to the user
    // 3. Return token details (without the actual token value)
    
    // For now, return service unavailable
    Ok((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
        "error": "Token retrieval is temporarily unavailable due to database layer issues"
    }))))
}

//! API token authentication middleware
//!
//! Provides authentication middleware for API tokens using Bearer token authorization.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::app::AppState;
use crate::models::ApiToken;
use crate::util::HashedToken;

/// API token authentication context
#[derive(Debug, Clone)]
pub struct ApiTokenAuth {
    /// The user ID associated with the token
    pub user_id: u64,
    /// The token ID
    pub token_id: u64,
}

/// Authenticate a request using API token
///
/// This middleware checks for a Bearer token in the Authorization header,
/// validates it against the database, and extracts the user ID and token ID.
pub async fn api_token_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    use http::header::AUTHORIZATION;

    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if it's a Bearer token
    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token_str = &auth_header[7..]; // Remove "Bearer " prefix

    // Parse and hash the token
    let hashed_token = HashedToken::parse(token_str).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let mut db = state.0.database.db_clone();

    // Query api_tokens table by hashed token
    let mut api_token = ApiToken::filter(
        ApiToken::fields()
            .token()
            .eq(hashed_token.as_bytes().to_vec()),
    )
    .first()
    .exec(&mut db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if token is revoked
    if api_token.revoked {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Check if token is expired
    if !api_token.is_valid() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Update last_used_at timestamp
    let last_used_at = Some(jiff::Timestamp::now());
    toasty::update!(api_token { last_used_at })
        .exec(&mut db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Store auth context in request extensions
    let auth = ApiTokenAuth {
        user_id: api_token.user_id,
        token_id: api_token.id,
    };
    request.extensions_mut().insert(auth);

    Ok(next.run(request).await)
}

/// Extractor for API token authentication context
///
/// Use this in your handlers to get the authenticated user ID and token ID.
pub fn extract_api_token_auth(request: &Request) -> Option<&ApiTokenAuth> {
    request.extensions().get::<ApiTokenAuth>()
}

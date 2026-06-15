//! API token authentication middleware
//!
//! Provides authentication middleware for API tokens using Bearer token authorization.

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::app::AppState;
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
/// validates it, and extracts the user ID and token ID.
pub async fn api_token_auth(
    State(_state): State<AppState>,
    request: Request,
    _next: Next,
) -> Result<Response, StatusCode> {
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
    let hashed_token = HashedToken::parse(token_str)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // TODO: Look up token in database once Toasty proc macro ABI mismatch is resolved
    // For now, we'll implement a placeholder that validates the token format
    // Full implementation should:
    // 1. Query api_tokens table by hashed token
    // 2. Check if token is not revoked and not expired
    // 3. Check if user account is not locked (account_lock_until > now)
    // 4. If locked, return forbidden with lock reason
    // 5. Update last_used_at timestamp
    // 6. Extract user_id and token_id

    // Placeholder: For now, we'll just validate the token format
    // In production, this would be a database lookup
    let _ = hashed_token; // Use the variable to avoid unused warning

    // For now, return unauthorized since we can't validate against database
    // Once database layer is working, this will return the actual user_id and token_id
    return Err(StatusCode::SERVICE_UNAVAILABLE);

    // Once database is working, the code would look like:
    // let db = state.database.db();
    // let api_token = ApiToken::filter(ApiToken::F::token.equals(hashed_token.as_bytes()))
    //     .filter(ApiToken::F::revoked.equals(false))
    //     .first(db)
    //     .await
    //     .ok_or(StatusCode::UNAUTHORIZED)?;
    //
    // // Check if token is expired
    // if !api_token.is_valid() {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }
    //
    // // Check if user account is locked
    // let user = User::filter(User::F::id.equals(api_token.user_id))
    //     .first(db)
    //     .await
    //     .ok_or(StatusCode::UNAUTHORIZED)?;
    //
    // if let Some(lock_until) = user.account_lock_until {
    //     if lock_until > jiff::Timestamp::now() {
    //         return Err(StatusCode::FORBIDDEN);
    //     }
    // }
    //
    // // Update last_used_at
    // let mut token_update = api_token.clone();
    // token_update.last_used_at = Some(jiff::Timestamp::now());
    // token_update.update(db).await;
    //
    // // Store auth context in request extensions
    // let auth = ApiTokenAuth {
    //     user_id: api_token.user_id,
    //     token_id: api_token.id,
    // };
    // request.extensions_mut().insert(auth);
    //
    // Ok(next.run(request).await)
}

/// Extractor for API token authentication context
///
/// Use this in your handlers to get the authenticated user ID and token ID.
pub fn extract_api_token_auth(request: &Request) -> Option<&ApiTokenAuth> {
    request.extensions().get::<ApiTokenAuth>()
}

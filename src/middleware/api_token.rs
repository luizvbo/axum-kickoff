//! API token authentication middleware
//!
//! Provides authentication middleware for API tokens using Bearer token authorization.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use subtle::ConstantTimeEq;

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
    /// The full API token record (for scope validation)
    pub api_token: Arc<ApiToken>,
}

/// Require API token middleware
///
/// Returns a 401 Unauthorized error if the request does not have a valid API token.
/// This is a simpler version of api_token_auth that returns a Response directly
/// instead of a Result, making it easier to use with route_layer.
///
/// # Example
///
/// ```rust,no_run
/// let router = Router::new()
///     .route("/api/tokens", get(list_tokens))
///     .route_layer(middleware::from_fn_with_state(
///         app_state.clone(),
///         require_api_token
///     ));
/// ```
pub async fn require_api_token(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    use http::header::AUTHORIZATION;

    // Extract Authorization header
    let auth_header = match request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        Some(header) => header,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    // Check if it's a Bearer token
    if !auth_header.starts_with("Bearer ") {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let token_str = &auth_header[7..]; // Remove "Bearer " prefix

    // Parse and hash the token
    let hashed_token = match HashedToken::parse(token_str) {
        Ok(token) => token,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let mut db = state.0.database.db_clone();

    // Query api_tokens table by hashed token
    let mut api_token = match ApiToken::filter(
        ApiToken::fields()
            .token()
            .eq(hashed_token.as_bytes().to_vec()),
    )
    .first()
    .exec(&mut db)
    .await
    {
        Ok(Some(token)) => token,
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Constant-time comparison of the token hash to prevent timing attacks
    // This ensures that even if the database query timing varies, the final
    // comparison is constant-time
    let stored_hash: &[u8] = api_token.token.as_slice();
    let provided_hash: &[u8] = hashed_token.as_bytes();
    if !bool::from(stored_hash.ct_eq(provided_hash)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Check if token is revoked
    if api_token.revoked {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Check if token is expired
    if !api_token.is_valid() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Update last_used_at timestamp
    let last_used_at = Some(jiff::Timestamp::now());
    if toasty::update!(api_token { last_used_at })
        .exec(&mut db)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Store auth context in request extensions
    let auth = ApiTokenAuth {
        user_id: api_token.user_id,
        token_id: api_token.id,
        api_token: Arc::new(api_token),
    };
    request.extensions_mut().insert(auth);

    next.run(request).await
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

    // Constant-time comparison of the token hash to prevent timing attacks
    let stored_hash: &[u8] = api_token.token.as_slice();
    let provided_hash: &[u8] = hashed_token.as_bytes();
    if !bool::from(stored_hash.ct_eq(provided_hash)) {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
        api_token: Arc::new(api_token),
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

/// Type alias for CurrentUser using Axum's Extension extractor
///
/// This provides a convenient way to extract the current user from request extensions.
/// Use it as: `Extension<CurrentUser>` in your handler parameters.
pub type CurrentUser = ApiTokenAuth;

/// Type alias for CurrentAuth using Axum's Extension extractor
///
/// This provides a convenient way to extract the full authentication context
/// including the API token with its scopes. Use it as: `Extension<CurrentAuth>` in your handler parameters.
pub type CurrentAuth = ApiTokenAuth;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http::Request;
    use jiff::Timestamp;

    fn create_test_api_token(user_id: u64, token_id: u64) -> ApiToken {
        ApiToken {
            id: token_id,
            user_id,
            name: "test".to_string(),
            token: vec![1, 2, 3, 4],
            created_at: Timestamp::now(),
            last_used_at: None,
            revoked: false,
            resource_scopes: None,
            action_scopes: None,
            expired_at: None,
        }
    }

    #[test]
    fn test_api_token_auth_debug() {
        let api_token = create_test_api_token(123, 456);
        let auth = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
            api_token: Arc::new(api_token),
        };
        let debug_str = format!("{:?}", auth);
        assert!(debug_str.contains("123"));
        assert!(debug_str.contains("456"));
    }

    #[test]
    fn test_api_token_auth_clone() {
        let api_token = create_test_api_token(123, 456);
        let auth = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
            api_token: Arc::new(api_token),
        };
        let cloned = auth.clone();
        assert_eq!(auth.user_id, cloned.user_id);
        assert_eq!(auth.token_id, cloned.token_id);
    }

    #[test]
    fn test_extract_api_token_auth_none() {
        let request = Request::builder().body(Body::empty()).unwrap();

        let auth = extract_api_token_auth(&request);
        assert!(auth.is_none());
    }

    #[test]
    fn test_extract_api_token_auth_some() {
        let api_token = create_test_api_token(123, 456);
        let auth = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
            api_token: Arc::new(api_token),
        };

        let mut request = Request::builder().body(Body::empty()).unwrap();
        request.extensions_mut().insert(auth);

        let extracted = extract_api_token_auth(&request);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().user_id, 123);
        assert_eq!(extracted.unwrap().token_id, 456);
    }

    #[test]
    fn test_api_token_auth_new() {
        let api_token = create_test_api_token(1, 1);
        let auth = ApiTokenAuth {
            user_id: 1,
            token_id: 1,
            api_token: Arc::new(api_token),
        };
        assert_eq!(auth.user_id, 1);
        assert_eq!(auth.token_id, 1);
    }

    #[test]
    fn test_api_token_auth_large_ids() {
        let api_token = create_test_api_token(u64::MAX, u64::MAX);
        let auth = ApiTokenAuth {
            user_id: u64::MAX,
            token_id: u64::MAX,
            api_token: Arc::new(api_token),
        };
        assert_eq!(auth.user_id, u64::MAX);
        assert_eq!(auth.token_id, u64::MAX);
    }

    #[test]
    fn test_api_token_auth_zero_ids() {
        let api_token = create_test_api_token(0, 0);
        let auth = ApiTokenAuth {
            user_id: 0,
            token_id: 0,
            api_token: Arc::new(api_token),
        };
        assert_eq!(auth.user_id, 0);
        assert_eq!(auth.token_id, 0);
    }

    #[test]
    fn test_extract_api_token_auth_multiple_extensions() {
        let api_token = create_test_api_token(999, 888);
        let auth = ApiTokenAuth {
            user_id: 999,
            token_id: 888,
            api_token: Arc::new(api_token),
        };

        let mut request = Request::builder().body(Body::empty()).unwrap();
        request.extensions_mut().insert(auth);
        request.extensions_mut().insert("other_data");

        let extracted = extract_api_token_auth(&request);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().user_id, 999);
    }

    #[test]
    fn test_extract_api_token_auth_wrong_type() {
        let mut request = Request::builder().body(Body::empty()).unwrap();
        request.extensions_mut().insert("not_an_auth");

        let extracted = extract_api_token_auth(&request);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_api_token_auth_eq() {
        let api_token1 = create_test_api_token(123, 456);
        let api_token2 = create_test_api_token(123, 456);
        let auth1 = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
            api_token: Arc::new(api_token1),
        };
        let auth2 = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
            api_token: Arc::new(api_token2),
        };
        // ApiTokenAuth doesn't derive PartialEq, so we can't test equality directly
        // Just verify the fields are the same
        assert_eq!(auth1.user_id, auth2.user_id);
        assert_eq!(auth1.token_id, auth2.token_id);
    }

    #[test]
    fn test_api_token_auth_different() {
        let api_token1 = create_test_api_token(123, 456);
        let api_token2 = create_test_api_token(789, 101);
        let auth1 = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
            api_token: Arc::new(api_token1),
        };
        let auth2 = ApiTokenAuth {
            user_id: 789,
            token_id: 101,
            api_token: Arc::new(api_token2),
        };
        assert_ne!(auth1.user_id, auth2.user_id);
        assert_ne!(auth1.token_id, auth2.token_id);
    }

    #[test]
    fn test_extract_api_token_auth_after_removal() {
        let api_token = create_test_api_token(111, 222);
        let auth = ApiTokenAuth {
            user_id: 111,
            token_id: 222,
            api_token: Arc::new(api_token),
        };

        let mut request = Request::builder().body(Body::empty()).unwrap();
        request.extensions_mut().insert(auth);

        let extracted = extract_api_token_auth(&request);
        assert!(extracted.is_some());

        // Remove the auth
        request.extensions_mut().remove::<ApiTokenAuth>();

        let extracted_after = extract_api_token_auth(&request);
        assert!(extracted_after.is_none());
    }

    #[test]
    fn test_extract_api_token_auth_empty_extensions() {
        let request = Request::builder().body(Body::empty()).unwrap();

        // Extensions should be empty initially
        let extracted = extract_api_token_auth(&request);
        assert!(extracted.is_none());
    }
}

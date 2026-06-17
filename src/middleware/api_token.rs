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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http::Request;

    #[test]
    fn test_api_token_auth_debug() {
        let auth = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
        };
        let debug_str = format!("{:?}", auth);
        assert!(debug_str.contains("123"));
        assert!(debug_str.contains("456"));
    }

    #[test]
    fn test_api_token_auth_clone() {
        let auth = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
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
        let auth = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
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
        let auth = ApiTokenAuth {
            user_id: 1,
            token_id: 1,
        };
        assert_eq!(auth.user_id, 1);
        assert_eq!(auth.token_id, 1);
    }

    #[test]
    fn test_api_token_auth_large_ids() {
        let auth = ApiTokenAuth {
            user_id: u64::MAX,
            token_id: u64::MAX,
        };
        assert_eq!(auth.user_id, u64::MAX);
        assert_eq!(auth.token_id, u64::MAX);
    }

    #[test]
    fn test_api_token_auth_zero_ids() {
        let auth = ApiTokenAuth {
            user_id: 0,
            token_id: 0,
        };
        assert_eq!(auth.user_id, 0);
        assert_eq!(auth.token_id, 0);
    }

    #[test]
    fn test_extract_api_token_auth_multiple_extensions() {
        let auth = ApiTokenAuth {
            user_id: 999,
            token_id: 888,
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
        let auth1 = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
        };
        let auth2 = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
        };
        // ApiTokenAuth doesn't derive PartialEq, so we can't test equality directly
        // Just verify the fields are the same
        assert_eq!(auth1.user_id, auth2.user_id);
        assert_eq!(auth1.token_id, auth2.token_id);
    }

    #[test]
    fn test_api_token_auth_different() {
        let auth1 = ApiTokenAuth {
            user_id: 123,
            token_id: 456,
        };
        let auth2 = ApiTokenAuth {
            user_id: 789,
            token_id: 101,
        };
        assert_ne!(auth1.user_id, auth2.user_id);
        assert_ne!(auth1.token_id, auth2.token_id);
    }

    #[test]
    fn test_extract_api_token_auth_after_removal() {
        let auth = ApiTokenAuth {
            user_id: 111,
            token_id: 222,
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

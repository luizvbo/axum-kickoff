//! API token authentication middleware
//!
//! Provides authentication middleware for API tokens using Bearer token authorization.

use axum::extract::Request;
use std::sync::Arc;

use crate::models::ApiToken;

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

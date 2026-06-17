//! Authentication utilities
//!
//! Provides the AuthCheck pattern for endpoint-level authentication with scoped tokens.

use crate::models::token::{ResourceScope, ActionScope};
use crate::models::ApiToken;
use crate::util::errors::{forbidden, unauthorized, BoxedAppError};
use axum::extract::FromRequestParts;
use http::header;
use http::request::Parts;
use secrecy::SecretString;
use std::sync::Arc;

/// Authentication method (cookie session or API token)
#[derive(Debug, Clone)]
pub enum Authentication {
    /// Cookie-based session authentication
    Cookie {
        /// The user ID from the session
        user_id: u64,
    },
    /// API token authentication
    Token {
        /// The user ID associated with the token
        user_id: u64,
        /// The token ID
        token_id: u64,
        /// The API token with scopes
        api_token: Arc<ApiToken>,
    },
}

impl Authentication {
    /// Get the user ID regardless of authentication method
    pub fn user_id(&self) -> u64 {
        match self {
            Authentication::Cookie { user_id } => *user_id,
            Authentication::Token { user_id, .. } => *user_id,
        }
    }

    /// Get the API token if authenticated via token
    pub fn api_token(&self) -> Option<&Arc<ApiToken>> {
        match self {
            Authentication::Cookie { .. } => None,
            Authentication::Token { api_token, .. } => Some(api_token),
        }
    }
}

/// Authorization header extractor
#[derive(Debug)]
pub struct AuthHeader(SecretString);

impl AuthHeader {
    /// Extract authorization header from request parts (optional)
    pub async fn optional_from_request_parts(parts: &Parts) -> Result<Option<Self>, BoxedAppError> {
        let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) else {
            return Ok(None);
        };

        let auth_header = auth_header.to_str().map_err(|_| {
            let message = "Invalid `Authorization` header: Found unexpected non-ASCII characters";
            unauthorized(message)
        })?;

        let (scheme, token) = auth_header.split_once(' ').unwrap_or(("", auth_header));
        if !(scheme.eq_ignore_ascii_case("Bearer") || scheme.is_empty()) {
            let message = format!(
                "Invalid `Authorization` header: Found unexpected authentication scheme `{scheme}`"
            );
            return Err(unauthorized(message));
        }

        let token = SecretString::from(token.trim_ascii());
        Ok(Some(AuthHeader(token)))
    }

    /// Extract authorization header from request parts (required)
    pub async fn from_request_parts(parts: &Parts) -> Result<Self, BoxedAppError> {
        let auth = Self::optional_from_request_parts(parts).await?;
        auth.ok_or_else(|| {
            let message = "Missing `Authorization` header";
            unauthorized(message)
        })
    }

    /// Get the token value
    pub fn token(&self) -> &SecretString {
        &self.0
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthHeader {
    type Rejection = BoxedAppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Self::from_request_parts(parts).await
    }
}

/// Authentication check configuration
///
/// This struct configures authentication requirements for an endpoint,
/// including whether API tokens are allowed and what scopes are required.
#[derive(Debug, Clone)]
pub struct AuthCheck {
    /// Whether API token authentication is allowed
    allow_token: bool,
    /// Required action scope (if any)
    action_scope: Option<ActionScope>,
    /// Required resource name (if endpoint deals with specific resources)
    crate_name: Option<String>,
    /// Allow tokens with any resource scope without specifying a particular resource
    allow_any_crate_scope: bool,
}

impl Default for AuthCheck {
    fn default() -> Self {
        Self {
            allow_token: true,
            action_scope: None,
            crate_name: None,
            allow_any_crate_scope: false,
        }
    }
}

impl AuthCheck {
    /// Create a default AuthCheck that allows both cookies and tokens
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an AuthCheck that only allows cookie authentication
    #[must_use]
    pub fn only_cookie() -> Self {
        Self {
            allow_token: false,
            action_scope: None,
            crate_name: None,
            allow_any_crate_scope: false,
        }
    }

    /// Set the required action scope
    #[must_use]
    pub fn with_action_scope(&self, action_scope: ActionScope) -> Self {
        Self {
            allow_token: self.allow_token,
            action_scope: Some(action_scope),
            crate_name: self.crate_name.clone(),
            allow_any_crate_scope: self.allow_any_crate_scope,
        }
    }

    /// Set the required resource name
    #[must_use]
    pub fn for_crate(&self, resource_name: &str) -> Self {
        Self {
            allow_token: self.allow_token,
            action_scope: self.action_scope,
            crate_name: Some(resource_name.to_string()),
            allow_any_crate_scope: self.allow_any_crate_scope,
        }
    }

    /// Allow tokens with any resource scope without specifying a particular resource
    ///
    /// Use this for endpoints that deal with multiple resources at once, where the
    /// caller will handle resource scope filtering manually.
    #[must_use]
    pub fn allow_any_crate_scope(&self) -> Self {
        Self {
            allow_token: self.allow_token,
            action_scope: self.action_scope,
            crate_name: self.crate_name.clone(),
            allow_any_crate_scope: true,
        }
    }

    /// Check if the authentication meets the requirements
    ///
    /// This validates that:
    /// - If the endpoint doesn't allow tokens, the auth must be cookie-based
    /// - If an endpoint scope is required, the token must have that scope
    /// - If a resource name is specified, the token must have a matching resource scope
    pub fn check(&self, auth: &Authentication) -> Result<(), BoxedAppError> {
        if let Some(token) = auth.api_token() {
            if !self.allow_token {
                return Err(forbidden(
                    "this action can only be performed on the website",
                ));
            }

            if !self.action_scope_matches(token.parse_action_scopes().as_deref()) {
                return Err(forbidden(
                    "this token does not have the required permissions to perform this action",
                ));
            }

            if !self.resource_scope_matches(token.parse_resource_scopes().as_deref()) {
                return Err(forbidden(
                    "this token does not have the required permissions to perform this action",
                ));
            }
        }

        Ok(())
    }

    /// Check if the token's action scopes match the required scope
    fn action_scope_matches(&self, token_scopes: Option<&[ActionScope]>) -> bool {
        match (&token_scopes, &self.action_scope) {
            // The token is a legacy token (no scopes)
            (None, _) => true,

            // The token is NOT a legacy token, and the endpoint only allows legacy tokens
            (Some(_), None) => false,

            // The token is NOT a legacy token, and the endpoint allows a certain action scope
            (Some(token_scopes), Some(action_scope)) => token_scopes.contains(action_scope),
        }
    }

    /// Check if the token's resource scopes match the required resource
    fn resource_scope_matches(&self, token_scopes: Option<&[ResourceScope]>) -> bool {
        match (&token_scopes, &self.crate_name) {
            // The token is a legacy token (no scopes)
            (None, _) => true,

            // The token does not have any resource scopes
            (Some([]), _) => true,

            // The token has resource scopes, but the endpoint does not deal with specific resources
            // However, if allow_any_crate_scope is set, we allow it (caller handles filtering)
            (Some(_), None) => self.allow_any_crate_scope,

            // The token is NOT a legacy token, and the endpoint requires a specific resource
            (Some(token_scopes), Some(resource_name)) => token_scopes
                .iter()
                .any(|token_scope| token_scope.matches(resource_name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::token::{ApiToken, ActionScope};
    use jiff::Timestamp;
    use std::sync::Arc;

    fn create_test_token(
        action_scopes: Option<Vec<ActionScope>>,
        resource_scopes: Option<Vec<String>>,
    ) -> Arc<ApiToken> {
        let resource_scopes_json = resource_scopes.and_then(|scopes| serde_json::to_string(&scopes).ok());
        let action_scopes_json =
            action_scopes.and_then(|scopes| serde_json::to_string(&scopes).ok());

        Arc::new(ApiToken {
            id: 1,
            user_id: 1,
            name: "test".to_string(),
            token: vec![],
            created_at: Timestamp::now(),
            last_used_at: None,
            revoked: false,
            resource_scopes: resource_scopes_json,
            action_scopes: action_scopes_json,
            expired_at: None,
        })
    }

    #[test]
    fn test_auth_check_default() {
        let check = AuthCheck::new();
        assert!(check.allow_token);
        assert!(check.action_scope.is_none());
        assert!(check.crate_name.is_none());
        assert!(!check.allow_any_crate_scope);
    }

    #[test]
    fn test_auth_check_only_cookie() {
        let check = AuthCheck::only_cookie();
        assert!(!check.allow_token);
    }

    #[test]
    fn test_auth_check_with_action_scope() {
        let check = AuthCheck::new().with_action_scope(ActionScope::Read);
        assert_eq!(check.action_scope, Some(ActionScope::Read));
    }

    #[test]
    fn test_auth_check_for_resource() {
        let check = AuthCheck::new().for_crate("posts");
        assert_eq!(check.crate_name, Some("posts".to_string()));
    }

    #[test]
    fn test_auth_check_allow_any_crate_scope() {
        let check = AuthCheck::new().allow_any_crate_scope();
        assert!(check.allow_any_crate_scope);
    }

    #[test]
    fn test_action_scope_matches_legacy_token() {
        let check = AuthCheck::new().with_action_scope(ActionScope::Read);
        let token = create_test_token(None, None);
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_ok());
    }

    #[test]
    fn test_action_scope_matches_with_scope() {
        let check = AuthCheck::new().with_action_scope(ActionScope::Read);
        let token = create_test_token(Some(vec![ActionScope::Read]), None);
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_ok());
    }

    #[test]
    fn test_action_scope_mismatch() {
        let check = AuthCheck::new().with_action_scope(ActionScope::Read);
        let token = create_test_token(Some(vec![ActionScope::Create]), None);
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_err());
    }

    #[test]
    fn test_crate_scope_matches_legacy_token() {
        let check = AuthCheck::new().for_crate("posts");
        let token = create_test_token(None, None);
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_ok());
    }

    #[test]
    fn test_crate_scope_matches_exact() {
        let check = AuthCheck::new().for_crate("posts");
        let token = create_test_token(None, Some(vec!["posts".to_string()]));
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_ok());
    }

    #[test]
    fn test_crate_scope_matches_wildcard() {
        let check = AuthCheck::new().for_crate("posts");
        let token = create_test_token(None, Some(vec!["p*".to_string()]));
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_ok());
    }

    #[test]
    fn test_crate_scope_mismatch() {
        let check = AuthCheck::new().for_crate("posts");
        let token = create_test_token(None, Some(vec!["users".to_string()]));
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_err());
    }

    #[test]
    fn test_token_disallowed() {
        let check = AuthCheck::only_cookie();
        let token = create_test_token(None, None);
        let auth = Authentication::Token {
            user_id: 1,
            token_id: 1,
            api_token: token,
        };
        assert!(check.check(&auth).is_err());
    }

    #[test]
    fn test_cookie_auth_always_allowed() {
        let check = AuthCheck::new().with_action_scope(ActionScope::Read);
        let auth = Authentication::Cookie { user_id: 1 };
        assert!(check.check(&auth).is_ok());
    }
}

//! API token management controller
//!
//! Provides endpoints for creating, listing, and revoking API tokens.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::middleware::SessionExtension;
use crate::models::token::{ActionScope, ResourceScope};
use crate::models::ApiToken;
use crate::util::errors::{bad_request, server_error, unauthorized, AppResult};
use crate::util::PlainToken;

/// Request body for creating a new API token
#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    /// The name of the token
    pub name: String,
    /// Optional resource scope patterns (as strings, will be validated)
    pub resource_scopes: Option<Vec<String>>,
    /// Optional action scopes (as strings, will be validated)
    pub action_scopes: Option<Vec<String>>,
    /// Optional expiration date (ISO 8601 format)
    pub expires_at: Option<String>,
}

impl CreateTokenRequest {
    const MAX_TOKEN_NAME_LENGTH: usize = 100;

    /// Validate and convert the request into validated types
    pub fn validate(self) -> Result<ValidatedCreateTokenRequest, String> {
        // Trim and validate name
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err("Token name cannot be empty".to_string());
        }
        if name.len() > Self::MAX_TOKEN_NAME_LENGTH {
            return Err(format!(
                "Token name cannot exceed {} characters",
                Self::MAX_TOKEN_NAME_LENGTH
            ));
        }

        // Validate resource scopes
        let resource_scopes = self
            .resource_scopes
            .map(|scopes| {
                scopes
                    .into_iter()
                    .map(|s| ResourceScope::try_from(s))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|e| format!("Invalid resource scope: {}", e))?;

        // Validate action scopes
        let action_scopes = self
            .action_scopes
            .map(|scopes| {
                scopes
                    .into_iter()
                    .map(|s| ActionScope::parse(&s))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|e| format!("Invalid action scope: {}", e))?;

        // Validate expiration date
        let expires_at = if let Some(s) = self.expires_at {
            let timestamp = jiff::Timestamp::strptime("%Y-%m-%dT%H:%M:%SZ", &s).map_err(|_| {
                "Invalid expires_at format. Use ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ".to_string()
            })?;
            if timestamp < jiff::Timestamp::now() {
                return Err("Expiration date cannot be in the past".to_string());
            }
            Some(timestamp)
        } else {
            None
        };

        Ok(ValidatedCreateTokenRequest {
            name,
            resource_scopes,
            action_scopes,
            expires_at,
        })
    }
}

/// Validated token creation request with typed scopes
#[derive(Debug)]
pub struct ValidatedCreateTokenRequest {
    pub name: String,
    pub resource_scopes: Option<Vec<ResourceScope>>,
    pub action_scopes: Option<Vec<ActionScope>>,
    pub expires_at: Option<jiff::Timestamp>,
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
    /// Resource scopes
    pub resource_scopes: Option<Vec<String>>,
    /// Action scopes
    pub action_scopes: Option<Vec<String>>,
    /// Expiration date
    pub expires_at: Option<String>,
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
    /// Resource scopes
    pub resource_scopes: Option<Vec<String>>,
    /// Action scopes
    pub action_scopes: Option<Vec<String>>,
    /// Expiration date
    pub expires_at: Option<String>,
}

/// Create a new API token
///
/// This endpoint creates a new API token for the authenticated user.
/// The token is returned in plain text and should be stored securely by the client.
pub async fn create_token(
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
    Json(req): Json<CreateTokenRequest>,
) -> AppResult<impl IntoResponse> {
    let user_id = session
        .get("user_id")
        .ok_or_else(|| unauthorized("Not logged in"))?;
    let user_id = user_id
        .parse::<u64>()
        .map_err(|_| unauthorized("Invalid session"))?;

    // Validate the request
    let validated = req.validate().map_err(|e| bad_request(e))?;

    let plain_token = PlainToken::generate();
    let hashed_token = plain_token.hashed();

    // Serialize scopes to JSON for storage
    let resource_scopes_json = validated
        .resource_scopes
        .as_ref()
        .and_then(|scopes| serde_json::to_string(scopes).ok());
    let action_scopes_json = validated
        .action_scopes
        .as_ref()
        .and_then(|scopes| serde_json::to_string(scopes).ok());

    let mut db = state.0.database.db_clone();

    let token_record = toasty::create!(ApiToken {
        user_id,
        name: validated.name.clone(),
        token: hashed_token.as_bytes().to_vec(),
        created_at: jiff::Timestamp::now(),
        last_used_at: None,
        revoked: false,
        resource_scopes: resource_scopes_json,
        action_scopes: action_scopes_json,
        expired_at: validated.expires_at,
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    // Convert scopes back to strings for response
    let resource_scopes_response = validated.resource_scopes.map(|scopes| {
        scopes
            .into_iter()
            .map(|s| s.pattern().to_string())
            .collect()
    });
    let action_scopes_response = validated
        .action_scopes
        .map(|scopes| scopes.into_iter().map(|s| s.as_str().to_string()).collect());
    let expires_at_response = validated.expires_at.map(|t| t.to_string());

    use secrecy::ExposeSecret;
    Ok((
        StatusCode::CREATED,
        Json(CreateTokenResponse {
            token: plain_token.expose_secret().to_string(),
            id: token_record.id,
            name: token_record.name,
            created_at: token_record.created_at.to_string(),
            resource_scopes: resource_scopes_response,
            action_scopes: action_scopes_response,
            expires_at: expires_at_response,
        }),
    ))
}

/// List all API tokens for the authenticated user
///
/// This endpoint returns a list of all API tokens belonging to the authenticated user.
pub async fn list_tokens(
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
) -> AppResult<impl IntoResponse> {
    let user_id = session
        .get("user_id")
        .ok_or_else(|| unauthorized("Not logged in"))?;
    let user_id = user_id
        .parse::<u64>()
        .map_err(|_| unauthorized("Invalid session"))?;

    let mut db = state.0.database.db_clone();

    // Query all tokens for the user using Toasty's filter API
    let tokens = ApiToken::filter(ApiToken::fields().user_id().eq(user_id))
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let token_list: Vec<TokenListItem> = tokens
        .into_iter()
        .map(|token| {
            let resource_scopes = token.parse_resource_scopes().map(|scopes| {
                scopes
                    .into_iter()
                    .map(|s| s.pattern().to_string())
                    .collect()
            });
            let action_scopes = token
                .parse_action_scopes()
                .map(|scopes| scopes.into_iter().map(|s| s.as_str().to_string()).collect());

            TokenListItem {
                id: token.id,
                name: token.name,
                created_at: token.created_at.to_string(),
                last_used_at: token.last_used_at.map(|t| t.to_string()),
                revoked: token.revoked,
                resource_scopes,
                action_scopes,
                expires_at: token.expired_at.map(|t| t.to_string()),
            }
        })
        .collect();

    Ok(Json(token_list))
}

/// Revoke an API token
///
/// This endpoint revokes (deletes) an API token by ID.
pub async fn revoke_token(
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
    Path(token_id): Path<u64>,
) -> AppResult<impl IntoResponse> {
    let user_id = session
        .get("user_id")
        .ok_or_else(|| unauthorized("Not logged in"))?;
    let user_id = user_id
        .parse::<u64>()
        .map_err(|_| unauthorized("Invalid session"))?;

    let mut db = state.0.database.db_clone();

    // Find the token and verify it belongs to the user
    let token = ApiToken::filter(ApiToken::fields().id().eq(token_id))
        .filter(ApiToken::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?
        .ok_or_else(|| server_error("API token not found".to_string()))?;

    // Mark the token as revoked using toasty::update!
    let mut token = token;
    toasty::update!(token { revoked: true })
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_token_request_deserialize() {
        let json = r#"{
            "name": "test-token",
            "resource_scopes": ["crate1", "crate2"],
            "action_scopes": ["api1", "api2"],
            "expires_at": "2024-12-31T23:59:59Z"
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert_eq!(
            req.resource_scopes,
            Some(vec!["crate1".to_string(), "crate2".to_string()])
        );
        assert_eq!(
            req.action_scopes,
            Some(vec!["api1".to_string(), "api2".to_string()])
        );
        assert_eq!(req.expires_at, Some("2024-12-31T23:59:59Z".to_string()));
    }

    #[test]
    fn test_create_token_request_minimal() {
        let json = r#"{"name": "test-token"}"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert!(req.resource_scopes.is_none());
        assert!(req.action_scopes.is_none());
        assert!(req.expires_at.is_none());
    }

    #[test]
    fn test_create_token_request_empty_scopes() {
        let json = r#"{
            "name": "test-token",
            "resource_scopes": [],
            "action_scopes": []
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert_eq!(req.resource_scopes, Some(vec![]));
        assert_eq!(req.action_scopes, Some(vec![]));
    }

    #[test]
    fn test_create_token_request_invalid_json() {
        let json = r#"{"name": "test-token", "invalid": "field"}"#;
        // Extra fields should be ignored by serde
        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
    }

    #[test]
    fn test_create_token_request_missing_name() {
        let json = r#"{"resource_scopes": ["crate1"]}"#;
        let req: Result<CreateTokenRequest, _> = serde_json::from_str(json);
        assert!(req.is_err());
    }

    #[test]
    fn test_create_token_response_serialize() {
        let response = CreateTokenResponse {
            token: "ako_test_token".to_string(),
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            resource_scopes: Some(vec!["crate1".to_string()]),
            action_scopes: Some(vec!["api1".to_string()]),
            expires_at: Some("2024-12-31T23:59:59Z".to_string()),
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("test-token"));
        assert!(json.contains("ako_test_token"));
    }

    #[test]
    fn test_create_token_response_minimal() {
        let response = CreateTokenResponse {
            token: "ako_test_token".to_string(),
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            resource_scopes: None,
            action_scopes: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("test-token"));
        assert!(json.contains("ako_test_token"));
    }

    #[test]
    fn test_token_list_item_serialize() {
        let item = TokenListItem {
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_used_at: Some("2024-01-02T00:00:00Z".to_string()),
            revoked: false,
            resource_scopes: Some(vec!["crate1".to_string()]),
            action_scopes: Some(vec!["api1".to_string()]),
            expires_at: Some("2024-12-31T23:59:59Z".to_string()),
        };

        let json = serde_json::to_string(&item).expect("Failed to serialize");
        assert!(json.contains("test-token"));
        assert!(json.contains("last_used_at"));
    }

    #[test]
    fn test_token_list_item_no_last_used() {
        let item = TokenListItem {
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_used_at: None,
            revoked: false,
            resource_scopes: None,
            action_scopes: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&item).expect("Failed to serialize");
        assert!(json.contains("test-token"));
    }

    #[test]
    fn test_token_list_item_revoked() {
        let item = TokenListItem {
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_used_at: Some("2024-01-02T00:00:00Z".to_string()),
            revoked: true,
            resource_scopes: None,
            action_scopes: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&item).expect("Failed to serialize");
        assert!(json.contains("\"revoked\":true"));
    }

    #[test]
    fn test_token_list_item_multiple_scopes() {
        let item = TokenListItem {
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_used_at: None,
            revoked: false,
            resource_scopes: Some(vec![
                "crate1".to_string(),
                "crate2".to_string(),
                "crate3".to_string(),
            ]),
            action_scopes: Some(vec!["api1".to_string(), "api2".to_string()]),
            expires_at: None,
        };

        let json = serde_json::to_string(&item).expect("Failed to serialize");
        assert!(json.contains("crate1"));
        assert!(json.contains("crate2"));
        assert!(json.contains("api1"));
    }

    #[test]
    fn test_create_token_request_serialize() {
        let req = CreateTokenRequest {
            name: "test-token".to_string(),
            resource_scopes: Some(vec!["crate1".to_string()]),
            action_scopes: Some(vec!["api1".to_string()]),
            expires_at: Some("2024-12-31T23:59:59Z".to_string()),
        };

        // CreateTokenRequest doesn't need to be serialized in production,
        // but we can test the fields are set correctly
        assert_eq!(req.name, "test-token");
        assert!(req.resource_scopes.is_some());
    }

    #[test]
    fn test_create_token_response_fields() {
        let response = CreateTokenResponse {
            token: "ako_test_token".to_string(),
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            resource_scopes: Some(vec!["crate1".to_string()]),
            action_scopes: Some(vec!["api1".to_string()]),
            expires_at: Some("2024-12-31T23:59:59Z".to_string()),
        };

        // Test that response fields are set correctly
        assert_eq!(response.token, "ako_test_token");
        assert_eq!(response.id, 123);
        assert_eq!(response.name, "test-token");
        assert_eq!(response.created_at, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_create_token_request_with_special_chars() {
        let json = r#"{
            "name": "token-with-special-chars_123",
            "resource_scopes": ["crate*test", "crate?pattern"]
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "token-with-special-chars_123");
        assert_eq!(
            req.resource_scopes,
            Some(vec!["crate*test".to_string(), "crate?pattern".to_string()])
        );
    }

    #[test]
    fn test_create_token_request_unicode_name() {
        let json = r#"{"name": "token-测试-🎉"}"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "token-测试-🎉");
    }

    #[test]
    fn test_create_token_request_very_long_name() {
        let long_name = "a".repeat(1000);
        let json = format!(r#"{{"name": "{}"}}"#, long_name);

        let req: CreateTokenRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(req.name.len(), 1000);
    }

    #[test]
    fn test_create_token_request_null_scopes() {
        let json = r#"{
            "name": "test-token",
            "resource_scopes": null,
            "action_scopes": null
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert!(req.resource_scopes.is_none());
        assert!(req.action_scopes.is_none());
    }

    #[test]
    fn test_token_list_item_with_all_fields() {
        let item = TokenListItem {
            id: 999,
            name: "comprehensive-token".to_string(),
            created_at: "2024-06-15T12:30:45Z".to_string(),
            last_used_at: Some("2024-06-16T08:15:30Z".to_string()),
            revoked: false,
            resource_scopes: Some(vec!["crate1".to_string(), "crate2".to_string()]),
            action_scopes: Some(vec![
                "api1".to_string(),
                "api2".to_string(),
                "api3".to_string(),
            ]),
            expires_at: Some("2025-06-15T12:30:45Z".to_string()),
        };

        let json = serde_json::to_string(&item).expect("Failed to serialize");
        assert!(json.contains("comprehensive-token"));
        assert!(json.contains("999"));
        assert!(json.contains("last_used_at"));
        assert!(json.contains("expires_at"));
    }

    #[test]
    fn test_token_list_item_large_id() {
        let item = TokenListItem {
            id: u64::MAX,
            name: "max-id-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_used_at: None,
            revoked: false,
            resource_scopes: None,
            action_scopes: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&item).expect("Failed to serialize");
        assert!(json.contains(&u64::MAX.to_string()));
    }

    #[test]
    fn test_create_token_response_roundtrip() {
        let original = CreateTokenResponse {
            token: "ako_roundtrip_test".to_string(),
            id: 456,
            name: "roundtrip-token".to_string(),
            created_at: "2024-03-15T10:20:30Z".to_string(),
            resource_scopes: Some(vec!["scope1".to_string(), "scope2".to_string()]),
            action_scopes: Some(vec!["endpoint1".to_string()]),
            expires_at: Some("2025-03-15T10:20:30Z".to_string()),
        };

        let json = serde_json::to_string(&original).expect("Failed to serialize");
        // CreateTokenResponse doesn't need to be deserialized in production
        // Just verify serialization works correctly
        assert!(json.contains("roundtrip-token"));
        assert!(json.contains("ako_roundtrip_test"));
    }

    #[test]
    fn test_token_list_item_roundtrip() {
        let original = TokenListItem {
            id: 789,
            name: "list-roundtrip".to_string(),
            created_at: "2024-04-20T15:45:00Z".to_string(),
            last_used_at: Some("2024-04-21T09:30:00Z".to_string()),
            revoked: true,
            resource_scopes: Some(vec!["test".to_string()]),
            action_scopes: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&original).expect("Failed to serialize");
        // TokenListItem doesn't need to be deserialized in production
        // Just verify serialization works correctly
        assert!(json.contains("list-roundtrip"));
        assert!(json.contains("\"revoked\":true"));
    }

    #[test]
    fn test_create_token_request_with_whitespace() {
        let json = r#"{
            "name": "  token with spaces  ",
            "resource_scopes": ["  scope1  ", "scope2"]
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "  token with spaces  ");
        assert_eq!(
            req.resource_scopes,
            Some(vec!["  scope1  ".to_string(), "scope2".to_string()])
        );
    }

    #[test]
    fn test_create_token_response_empty_token() {
        let response = CreateTokenResponse {
            token: "".to_string(),
            id: 1,
            name: "empty-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            resource_scopes: None,
            action_scopes: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"token\":\"\""));
    }
}

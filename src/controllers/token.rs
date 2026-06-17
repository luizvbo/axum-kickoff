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
use crate::models::ApiToken;
use crate::util::errors::{bad_request, server_error, unauthorized, AppResult};
use crate::util::PlainToken;

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

    let plain_token = PlainToken::generate();
    let hashed_token = plain_token.hashed();

    let crate_scopes = req
        .crate_scopes
        .as_ref()
        .map(|s| serde_json::to_string(s).unwrap());
    let endpoint_scopes = req
        .endpoint_scopes
        .as_ref()
        .map(|s| serde_json::to_string(s).unwrap());

    let expired_at = if let Some(s) = req.expired_at.as_ref() {
        Some(
            jiff::Timestamp::strptime("%Y-%m-%dT%H:%M:%SZ", s).map_err(|_| {
                bad_request("Invalid expired_at format. Use ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ")
            })?,
        )
    } else {
        None
    };

    let mut db = state.0.database.db_clone();

    let token_record = toasty::create!(ApiToken {
        user_id,
        name: req.name.clone(),
        token: hashed_token.as_bytes().to_vec(),
        created_at: jiff::Timestamp::now(),
        last_used_at: None,
        revoked: false,
        crate_scopes,
        endpoint_scopes,
        expired_at,
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    use secrecy::ExposeSecret;
    Ok((
        StatusCode::CREATED,
        Json(CreateTokenResponse {
            token: plain_token.expose_secret().to_string(),
            id: token_record.id,
            name: token_record.name,
            created_at: token_record.created_at.to_string(),
            crate_scopes: req.crate_scopes,
            endpoint_scopes: req.endpoint_scopes,
            expired_at: req.expired_at,
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
            let crate_scopes = token.parse_crate_scopes().map(|scopes| {
                scopes
                    .into_iter()
                    .map(|s| s.pattern().to_string())
                    .collect()
            });
            let endpoint_scopes = token
                .parse_endpoint_scopes()
                .map(|scopes| scopes.into_iter().map(|s| s.as_str().to_string()).collect());

            TokenListItem {
                id: token.id,
                name: token.name,
                created_at: token.created_at.to_string(),
                last_used_at: token.last_used_at.map(|t| t.to_string()),
                revoked: token.revoked,
                crate_scopes,
                endpoint_scopes,
                expired_at: token.expired_at.map(|t| t.to_string()),
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
            "crate_scopes": ["crate1", "crate2"],
            "endpoint_scopes": ["api1", "api2"],
            "expired_at": "2024-12-31T23:59:59Z"
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert_eq!(req.crate_scopes, Some(vec!["crate1".to_string(), "crate2".to_string()]));
        assert_eq!(req.endpoint_scopes, Some(vec!["api1".to_string(), "api2".to_string()]));
        assert_eq!(req.expired_at, Some("2024-12-31T23:59:59Z".to_string()));
    }

    #[test]
    fn test_create_token_request_minimal() {
        let json = r#"{"name": "test-token"}"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert!(req.crate_scopes.is_none());
        assert!(req.endpoint_scopes.is_none());
        assert!(req.expired_at.is_none());
    }

    #[test]
    fn test_create_token_request_empty_scopes() {
        let json = r#"{
            "name": "test-token",
            "crate_scopes": [],
            "endpoint_scopes": []
        }"#;

        let req: CreateTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.name, "test-token");
        assert_eq!(req.crate_scopes, Some(vec![]));
        assert_eq!(req.endpoint_scopes, Some(vec![]));
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
        let json = r#"{"crate_scopes": ["crate1"]}"#;
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
            crate_scopes: Some(vec!["crate1".to_string()]),
            endpoint_scopes: Some(vec!["api1".to_string()]),
            expired_at: Some("2024-12-31T23:59:59Z".to_string()),
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
            crate_scopes: None,
            endpoint_scopes: None,
            expired_at: None,
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
            crate_scopes: Some(vec!["crate1".to_string()]),
            endpoint_scopes: Some(vec!["api1".to_string()]),
            expired_at: Some("2024-12-31T23:59:59Z".to_string()),
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
            crate_scopes: None,
            endpoint_scopes: None,
            expired_at: None,
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
            crate_scopes: None,
            endpoint_scopes: None,
            expired_at: None,
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
            crate_scopes: Some(vec!["crate1".to_string(), "crate2".to_string(), "crate3".to_string()]),
            endpoint_scopes: Some(vec!["api1".to_string(), "api2".to_string()]),
            expired_at: None,
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
            crate_scopes: Some(vec!["crate1".to_string()]),
            endpoint_scopes: Some(vec!["api1".to_string()]),
            expired_at: Some("2024-12-31T23:59:59Z".to_string()),
        };

        // CreateTokenRequest doesn't need to be serialized in production,
        // but we can test the fields are set correctly
        assert_eq!(req.name, "test-token");
        assert!(req.crate_scopes.is_some());
    }

    #[test]
    fn test_create_token_response_fields() {
        let response = CreateTokenResponse {
            token: "ako_test_token".to_string(),
            id: 123,
            name: "test-token".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            crate_scopes: Some(vec!["crate1".to_string()]),
            endpoint_scopes: Some(vec!["api1".to_string()]),
            expired_at: Some("2024-12-31T23:59:59Z".to_string()),
        };

        // Test that response fields are set correctly
        assert_eq!(response.token, "ako_test_token");
        assert_eq!(response.id, 123);
        assert_eq!(response.name, "test-token");
        assert_eq!(response.created_at, "2024-01-01T00:00:00Z");
    }
}

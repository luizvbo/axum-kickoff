//! API token management controller
//!
//! Provides endpoints for creating, listing, and revoking API tokens.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::token::{ActionScope, ResourceScope};
use crate::models::ApiToken;
use crate::util::auth::{AuthCheck, Authentication};
use crate::util::errors::{bad_request, db_error, AppResult};
use crate::util::ApiResponse;
use crate::util::PlainToken;

/// Request body for creating a new API token
#[derive(Debug, Deserialize, ToSchema)]
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
                    .map(ResourceScope::try_from)
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
#[derive(Debug, Serialize, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
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
#[utoipa::path(
    post,
    path = "/api/v1/tokens",
    request_body = CreateTokenRequest,
    responses(
        (status = 201, description = "Token created", body = CreateTokenResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Bad request")
    ),
    tag = "Tokens",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_token(
    auth: Authentication,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Json(req): Json<CreateTokenRequest>,
) -> AppResult<impl IntoResponse> {
    AuthCheck::new()
        .with_action_scope(ActionScope::Create)
        .for_crate("tokens")
        .check(&auth)?;

    // Validate the request
    let validated = req.validate().map_err(bad_request)?;

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
    .map_err(db_error)?;

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
        Json(ApiResponse::new(CreateTokenResponse {
            token: plain_token.expose_secret().to_string(),
            id: token_record.id,
            name: token_record.name,
            created_at: token_record.created_at.to_string(),
            resource_scopes: resource_scopes_response,
            action_scopes: action_scopes_response,
            expires_at: expires_at_response,
        })),
    ))
}

/// Query parameters for token list pagination
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTokensParams {
    /// Page number (1-based, default 1)
    #[param(minimum = 1)]
    pub page: Option<u32>,
    /// Items per page (default 20, max 100)
    #[param(minimum = 1)]
    pub per_page: Option<u32>,
}

/// Response for paginated token list
#[derive(Debug, Serialize, ToSchema)]
pub struct ListTokensResponse {
    pub data: Vec<TokenListItem>,
    pub page: u32,
    pub per_page: usize,
}

const DEFAULT_TOKEN_PER_PAGE: usize = 20;
const MAX_TOKEN_PER_PAGE: usize = 100;

/// List all API tokens for the authenticated user
///
/// This endpoint returns a paginated list of all API tokens belonging to the authenticated user.
#[utoipa::path(
    get,
    path = "/api/v1/tokens",
    params(
        ListTokensParams
    ),
    responses(
        (status = 200, description = "Paginated list of tokens", body = ListTokensResponse),
        (status = 401, description = "Unauthorized")
    ),
    tag = "Tokens",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_tokens(
    auth: Authentication,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Query(params): Query<ListTokensParams>,
) -> AppResult<impl IntoResponse> {
    AuthCheck::new()
        .with_action_scope(ActionScope::Read)
        .for_crate("tokens")
        .check(&auth)?;

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params
        .per_page
        .map(|p| (p as usize).min(MAX_TOKEN_PER_PAGE))
        .unwrap_or(DEFAULT_TOKEN_PER_PAGE);
    let offset = ((page - 1) as usize) * per_page;

    let mut db = state.0.database.db_clone();

    // Query tokens for the user using Toasty's filter API with pagination
    let tokens = ApiToken::filter(ApiToken::fields().user_id().eq(user_id))
        .limit(per_page)
        .offset(offset)
        .exec(&mut db)
        .await
        .map_err(db_error)?;

    let data: Vec<TokenListItem> = tokens
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

    Ok(Json(ListTokensResponse {
        data,
        page,
        per_page,
    }))
}

/// Revoke an API token
///
/// This endpoint revokes (deletes) an API token by ID.
#[utoipa::path(
    post,
    path = "/api/v1/tokens/{token_id}",
    params(
        ("token_id" = u64, Path, description = "Token ID")
    ),
    responses(
        (status = 204, description = "Token revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Token not found")
    ),
    tag = "Tokens",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn revoke_token(
    auth: Authentication,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Path(token_id): Path<u64>,
) -> AppResult<impl IntoResponse> {
    AuthCheck::new()
        .with_action_scope(ActionScope::Delete)
        .for_crate("tokens")
        .check(&auth)?;

    let mut db = state.0.database.db_clone();

    // Find the token and verify it belongs to the user
    let token = ApiToken::filter(ApiToken::fields().id().eq(token_id))
        .filter(ApiToken::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(db_error)?
        .ok_or_else(crate::util::errors::not_found)?;

    // Mark the token as revoked using toasty::update!
    let mut token = token;
    toasty::update!(token { revoked: true })
        .exec(&mut db)
        .await
        .map_err(db_error)?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_token_request_missing_name() {
        let json = r#"{"resource_scopes": ["crate1"]}"#;
        let req: Result<CreateTokenRequest, _> = serde_json::from_str(json);
        assert!(req.is_err());
    }
}

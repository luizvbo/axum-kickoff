//! Authentication controller
//!
//! Handles GitHub OAuth authentication flow including authorize and callback endpoints.

use axum::extract::{Query, State, Extension};
use axum::response::{Json, Redirect};
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, CsrfToken, Scope, TokenUrl, TokenResponse};
use serde::Deserialize;
use serde_json::json;

use crate::app::AppState;
use crate::middleware::session::SessionExtension;
use crate::util::ReqwestClient;
use crate::util::errors::{BoxedAppError, bad_request};

/// OAuth authorize query parameters
#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    /// Optional redirect URL after successful authentication
    pub redirect_to: Option<String>,
}

/// OAuth callback query parameters
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    /// The authorization code from GitHub
    pub code: String,
    /// The state parameter for CSRF protection
    pub state: String,
}

/// GitHub user profile data from OAuth
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubUser {
    id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
}

/// GitHub OAuth authorize endpoint
///
/// Redirects the user to GitHub's OAuth authorization page.
/// The state parameter is stored in the session for CSRF protection.
///
/// # Example
///
/// ```
/// GET /api/v1/auth/github/authorize?redirect_to=/dashboard
/// ```
pub async fn github_authorize(
    Query(query): Query<AuthorizeQuery>,
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
) -> Result<Redirect, BoxedAppError> {
    let config = &state.0.config;
    
    // Create OAuth2 client
    let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
        .expect("Invalid authorization URL");
    let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
        .expect("Invalid token URL");
    
    let client = BasicClient::new(ClientId::new(config.gh_client_id.clone()))
        .set_client_secret(ClientSecret::new(config.gh_client_secret.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url);
    
    // Generate CSRF state token
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("read:user".to_string()))
        .add_scope(Scope::new("user:email".to_string()))
        .url();
    
    // Store CSRF token in session for verification on callback
    session.insert("github_oauth_state".to_string(), csrf_token.secret().clone());
    
    // Store redirect URL in session
    if let Some(redirect_to) = query.redirect_to {
        session.insert("redirect_to".to_string(), redirect_to);
    }
    
    Ok(Redirect::to(auth_url.as_str()))
}

/// GitHub OAuth callback endpoint
///
/// Handles the callback from GitHub after user authorization.
/// Exchanges the authorization code for an access token and fetches user profile.
///
/// # Example
///
/// ```
/// GET /api/v1/auth/github/callback?code=...&state=...
/// ```
pub async fn github_callback(
    Query(query): Query<CallbackQuery>,
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
) -> Result<Redirect, BoxedAppError> {
    let config = &state.0.config;
    
    // Verify CSRF state
    let session_state = session.remove("github_oauth_state")
        .ok_or_else(|| bad_request("Missing OAuth state in session"))?;
    
    if session_state != query.state {
        return Err(bad_request("Invalid OAuth state - possible CSRF attack"));
    }
    
    // Create OAuth2 client
    let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
        .expect("Invalid authorization URL");
    let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
        .expect("Invalid token URL");
    
    let client = BasicClient::new(ClientId::new(config.gh_client_id.clone()))
        .set_client_secret(ClientSecret::new(config.gh_client_secret.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url);
    
    // Exchange code for access token
    let token = client
        .exchange_code(oauth2::AuthorizationCode::new(query.code.clone()))
        .request_async(&ReqwestClient(reqwest::Client::new()))
        .await
        .map_err(|e| bad_request(format!("Failed to exchange authorization code: {}", e)))?;
    
    // Fetch user profile from GitHub
    let http_client = reqwest::Client::new();
    let user_response = http_client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token.access_token().secret()))
        .header("User-Agent", "axum-kickoff")
        .send()
        .await
        .map_err(|e| bad_request(format!("Failed to fetch user profile: {}", e)))?;
    
    if !user_response.status().is_success() {
        return Err(bad_request("Failed to fetch user profile from GitHub"));
    }
    
    let github_user: GitHubUser = user_response
        .json()
        .await
        .map_err(|e| bad_request(format!("Failed to parse user profile: {}", e)))?;
    
    // TODO: Create or update user in database
    // TODO: Create oauth_github record with encrypted token
    // TODO: Set user_id in session
    
    // For now, just store the GitHub user ID in session
    session.insert("user_id".to_string(), github_user.id.to_string());
    session.insert("user_login".to_string(), github_user.login.clone());
    
    // Redirect to the stored redirect URL or default to home
    let redirect_to = session.remove("redirect_to").unwrap_or_else(|| "/".to_string());
    
    Ok(Redirect::to(&redirect_to))
}

/// Logout endpoint
///
/// Clears the session and returns success.
///
/// # Example
///
/// ```
/// POST /api/v1/auth/logout
/// ```
pub async fn logout(
    Extension(session): Extension<SessionExtension>,
) -> Result<Json<serde_json::Value>, BoxedAppError> {
    // Clear all session data
    session.remove("user_id");
    session.remove("user_login");
    session.remove("github_oauth_state");
    session.remove("redirect_to");
    
    Ok(Json(json!({"success": true})))
}

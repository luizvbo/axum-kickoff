//! Authentication controller
//!
//! Handles the OAuth2 authorization-code flow (with PKCE) for every provider
//! enabled in the configuration, plus the login page and logout endpoints.
//! Providers are described by [`crate::oauth::OAuthProviderSpec`] and enabled
//! via environment credentials — see `config::Server::oauth_providers`.

use askama::Template;
use axum::extract::{Extension, Path, Query, State};
use axum::response::{Json, Redirect};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use serde_json::json;

use crate::app::AppState;
use crate::middleware::session::SessionExtension;
use crate::models::User;
use crate::router::{HtmlTemplate, PageContext};
use crate::util::errors::{bad_request, db_error, forbidden, server_error, BoxedAppError};
use crate::util::ReqwestClient;
use secrecy::ExposeSecret;

/// Session keys used while an OAuth flow is in progress. The provider slug is
/// recorded alongside the state so the callback can verify the flow was
/// started for the same provider that completed it.
const SESSION_OAUTH_STATE: &str = "oauth_state";
const SESSION_OAUTH_PKCE_VERIFIER: &str = "oauth_pkce_verifier";
const SESSION_OAUTH_PROVIDER: &str = "oauth_provider";
const SESSION_REDIRECT_TO: &str = "redirect_to";

/// OAuth authorize query parameters
#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    /// Optional redirect URL after successful authentication
    pub redirect_to: Option<String>,
}

/// OAuth callback query parameters
///
/// `code`/`state` are optional because providers redirect back with
/// `?error=...` (e.g. `error=access_denied`) instead when the user declines
/// authorization or the OAuth app is misconfigured.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    /// The authorization code from the provider
    pub code: Option<String>,
    /// The state parameter for CSRF protection
    pub state: Option<String>,
    /// Error code returned by the provider when authorization fails
    pub error: Option<String>,
    /// Human-readable error description returned by the provider
    pub error_description: Option<String>,
}

/// OAuth authorize endpoint
///
/// Redirects the user to the provider's OAuth authorization page.
/// Uses PKCE (Proof Key for Code Exchange) for enhanced security.
/// The state parameter and provider slug are stored in the session for CSRF
/// protection.
///
/// # Example
///
/// `GET /api/v1/auth/github/authorize?redirect_to=/dashboard`
pub async fn oauth_authorize(
    Path(provider_slug): Path<String>,
    Query(query): Query<AuthorizeQuery>,
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
) -> Result<Redirect, BoxedAppError> {
    let config = &state.0.config;
    let provider = config
        .oauth_provider(&provider_slug)
        .ok_or_else(|| bad_request("Unknown or unconfigured OAuth provider"))?;

    // Create OAuth2 client
    let auth_url =
        AuthUrl::new(provider.spec.authorize_url.to_string()).expect("Invalid authorization URL");
    let token_url = TokenUrl::new(provider.spec.token_url.to_string()).expect("Invalid token URL");
    let redirect_url =
        RedirectUrl::new(provider.redirect_uri.clone()).expect("Invalid redirect URL");

    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(
            provider.client_secret.expose_secret().to_string(),
        ))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    // Generate PKCE code verifier and challenge (supported by all configured
    // providers: GitHub, Google, and Facebook's OIDC code flow)
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate CSRF state token
    let mut authorize_request = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_code_challenge);
    for scope in provider.spec.scopes {
        authorize_request = authorize_request.add_scope(Scope::new(scope.to_string()));
    }
    let (auth_url, csrf_token) = authorize_request.url();

    // Store CSRF token in session for verification on callback
    session.insert(SESSION_OAUTH_STATE.to_string(), csrf_token.secret().clone());

    // Store PKCE code verifier in session for token exchange
    session.insert(
        SESSION_OAUTH_PKCE_VERIFIER.to_string(),
        pkce_code_verifier.secret().clone(),
    );

    // Record which provider this flow was started for, so the callback can
    // reject state tokens replayed against a different provider's endpoint.
    session.insert(
        SESSION_OAUTH_PROVIDER.to_string(),
        provider.spec.slug.to_string(),
    );

    // Store redirect URL in session (validate to prevent open redirect)
    if let Some(redirect_to) = query.redirect_to {
        // Validate redirect URL: must be relative or start with allowed domain
        if is_valid_redirect(&redirect_to, &config.domain_name) {
            session.insert(SESSION_REDIRECT_TO.to_string(), redirect_to);
        } else {
            tracing::warn!("Invalid redirect URL provided: {}", redirect_to);
        }
    }

    Ok(Redirect::to(auth_url.as_str()))
}

/// OAuth callback endpoint
///
/// Handles the callback from the provider after user authorization.
/// Verifies the CSRF state token and the recorded provider, exchanges the
/// authorization code for an access token using the PKCE verifier, then
/// fetches and stores the user profile.
///
/// # Example
///
/// `GET /api/v1/auth/github/callback?code=...&state=...`
pub async fn oauth_callback(
    Path(provider_slug): Path<String>,
    Query(query): Query<CallbackQuery>,
    State(state): State<AppState>,
    Extension(session): Extension<SessionExtension>,
) -> Result<Redirect, BoxedAppError> {
    let config = &state.0.config;
    let provider = config
        .oauth_provider(&provider_slug)
        .ok_or_else(|| bad_request("Unknown or unconfigured OAuth provider"))?;
    let provider_name = provider.spec.display_name;

    // Providers redirect here with `?error=...` when the user declines
    // authorization or the app is misconfigured, so `code`/`state` are
    // optional. Handle that first so the callback is a controlled response
    // instead of a query-deserialization 400.
    if let Some(error) = &query.error {
        tracing::warn!(
            provider = %provider.spec.slug,
            error = %error,
            error_description = query.error_description.as_deref().unwrap_or_default(),
            "OAuth callback returned an error"
        );
        return Err(bad_request(match error.as_str() {
            "access_denied" => format!("{provider_name} authorization was declined"),
            _ => format!("{provider_name} authorization failed, please try again"),
        }));
    }

    let (code, csrf_state) = match (query.code, query.state) {
        (Some(code), Some(state)) => (code, state),
        _ => return Err(bad_request("Missing OAuth code or state parameter")),
    };

    // Verify the provider recorded at authorize time matches this callback —
    // a state token issued for one provider must not complete a flow on
    // another.
    let session_provider = session
        .remove(SESSION_OAUTH_PROVIDER)
        .ok_or_else(|| bad_request("Missing OAuth state in session"))?;

    if session_provider != provider.spec.slug {
        return Err(bad_request(
            "OAuth provider mismatch - possible CSRF attack",
        ));
    }

    // Verify CSRF state
    let session_state = session
        .remove(SESSION_OAUTH_STATE)
        .ok_or_else(|| bad_request("Missing OAuth state in session"))?;

    if session_state != csrf_state {
        return Err(bad_request("Invalid OAuth state - possible CSRF attack"));
    }

    // Retrieve PKCE code verifier from session
    let pkce_verifier_secret = session
        .remove(SESSION_OAUTH_PKCE_VERIFIER)
        .ok_or_else(|| bad_request("Missing PKCE verifier in session"))?;
    let pkce_verifier = PkceCodeVerifier::new(pkce_verifier_secret);

    // Create OAuth2 client
    let auth_url =
        AuthUrl::new(provider.spec.authorize_url.to_string()).expect("Invalid authorization URL");
    let token_url = TokenUrl::new(provider.spec.token_url.to_string()).expect("Invalid token URL");
    let redirect_url =
        RedirectUrl::new(provider.redirect_uri.clone()).expect("Invalid redirect URL");

    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(
            provider.client_secret.expose_secret().to_string(),
        ))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    // Exchange code for access token with PKCE verifier
    let token = client
        .exchange_code(oauth2::AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&ReqwestClient(state.0.http_client.clone()))
        .await
        .map_err(|e| {
            tracing::error!(provider = %provider.spec.slug, error = %e, "Failed to exchange OAuth authorization code");
            server_error("Error obtaining token")
        })?;

    // Fetch user profile from the provider
    let user_response = state
        .0
        .http_client
        .get(provider.spec.userinfo_url)
        .header(
            "Authorization",
            format!("Bearer {}", token.access_token().secret()),
        )
        .header("User-Agent", "{{project-name}}")
        .send()
        .await
        .map_err(|e| {
            tracing::error!(provider = %provider.spec.slug, error = %e, "Failed to fetch OAuth user profile");
            server_error("Error obtaining user info")
        })?;

    if !user_response.status().is_success() {
        tracing::error!(
            provider = %provider.spec.slug,
            status = %user_response.status(),
            "OAuth user profile request returned an error status"
        );
        return Err(server_error("Error obtaining user info"));
    }

    let profile_body: serde_json::Value = user_response.json().await.map_err(|e| {
        tracing::error!(provider = %provider.spec.slug, error = %e, "Failed to parse OAuth user profile");
        server_error("Error obtaining user info")
    })?;

    let profile = (provider.spec.parse_profile)(&profile_body).ok_or_else(|| {
        tracing::error!(provider = %provider.spec.slug, "OAuth user profile is missing required fields");
        server_error("Error obtaining user info")
    })?;

    let mut db = state.0.database.db_clone();

    let user = match User::get_by_provider_and_provider_user_id(
        &mut db,
        provider.spec.slug,
        profile.provider_user_id.clone(),
    )
    .await
    {
        Ok(mut existing_user) => {
            // Deactivated accounts may not log back in
            if !existing_user.is_active {
                return Err(forbidden("Account is not active"));
            }

            // Check if locked
            if existing_user.is_locked() {
                let reason = existing_user
                    .account_lock_reason
                    .clone()
                    .unwrap_or_else(|| "Account is locked".into());
                return Err(forbidden(reason));
            }

            // Update existing user
            existing_user.update_from_oauth(
                profile.login.clone(),
                profile.name.clone(),
                profile.email.clone(),
                profile.avatar_url.clone(),
            );

            existing_user
                .update()
                .exec(&mut db)
                .await
                .map_err(db_error)?;

            existing_user
        }
        Err(_) => {
            // Create new user
            toasty::create!(User {
                provider: provider.spec.slug.to_string(),
                provider_user_id: profile.provider_user_id.clone(),
                login: profile.login.clone(),
                name: profile.name.clone(),
                email: profile.email.clone(),
                avatar_url: profile.avatar_url.clone(),
                is_active: true,
                account_lock_reason: None,
                account_lock_until: None,
                created_at: jiff::Timestamp::now(),
                updated_at: jiff::Timestamp::now(),
            })
            .exec(&mut db)
            .await
            .map_err(db_error)?
        }
    };

    // Session fixation protection: clear pre-login session data
    // except redirect_to, then set fresh authenticated session
    session.remove(SESSION_OAUTH_STATE);
    session.remove(SESSION_OAUTH_PKCE_VERIFIER);
    session.remove(SESSION_OAUTH_PROVIDER);
    session.remove("csrf_token");

    // Set user_id in session (this will trigger a fresh signed cookie)
    session.insert("user_id".to_string(), user.id.to_string());
    session.insert("user_login".to_string(), user.login);

    // Generate new CSRF token for the fresh session
    use crate::middleware::csrf::generate_token;
    session.insert("csrf_token".to_string(), generate_token());

    // Redirect to the stored redirect URL or default to home
    let redirect_to = session
        .remove(SESSION_REDIRECT_TO)
        .unwrap_or_else(|| "/".to_string());

    // Validate redirect URL before using it
    if !is_valid_redirect(&redirect_to, &config.domain_name) {
        tracing::warn!("Invalid redirect URL in session: {}", redirect_to);
        return Ok(Redirect::to("/"));
    }

    Ok(Redirect::to(&redirect_to))
}

/// Login page
///
/// Renders a sign-in button for every OAuth provider that has credentials
/// configured. The optional `redirect_to` query parameter is validated and
/// forwarded to the authorize endpoint so users land back where they started.
///
/// # Example
///
/// `GET /login?redirect_to=/dashboard`
pub async fn login_page(
    ctx: PageContext,
    Query(query): Query<AuthorizeQuery>,
) -> Result<HtmlTemplate<LoginTemplate>, BoxedAppError> {
    let redirect_to = match query.redirect_to {
        Some(url) if is_valid_redirect(&url, "") => url,
        _ => "/".to_string(),
    };

    Ok(HtmlTemplate::new(LoginTemplate { ctx, redirect_to }))
}

/// Login page template.
#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    ctx: PageContext,
    redirect_to: String,
}

/// Logout endpoint (API)
///
/// Clears the session and returns JSON success.
/// For HTML logout, use the /logout route which redirects.
///
/// # Example
///
/// `POST /api/v1/auth/logout`
pub async fn logout_api(
    Extension(session): Extension<SessionExtension>,
) -> Result<Json<serde_json::Value>, BoxedAppError> {
    // Clear all session data
    session.remove("user_id");
    session.remove("user_login");
    session.remove(SESSION_OAUTH_STATE);
    session.remove(SESSION_OAUTH_PKCE_VERIFIER);
    session.remove(SESSION_OAUTH_PROVIDER);
    session.remove(SESSION_REDIRECT_TO);
    session.remove("csrf_token");

    Ok(Json(json!({"success": true})))
}

/// Logout endpoint (HTML)
///
/// Clears the session and redirects to home.
/// This is for browser-based logout via forms.
///
/// # Example
///
/// `POST /logout`
pub async fn logout_html(
    Extension(session): Extension<SessionExtension>,
) -> Result<Redirect, BoxedAppError> {
    // Clear all session data
    session.remove("user_id");
    session.remove("user_login");
    session.remove(SESSION_OAUTH_STATE);
    session.remove(SESSION_OAUTH_PKCE_VERIFIER);
    session.remove(SESSION_OAUTH_PROVIDER);
    session.remove(SESSION_REDIRECT_TO);
    session.remove("csrf_token");

    Ok(Redirect::to("/"))
}

/// Validates a redirect URL to prevent open redirect attacks
///
/// Only allows relative URLs (starting with / but not //).
/// Absolute redirects are not permitted to prevent open redirect vulnerabilities.
fn is_valid_redirect(url: &str, _domain_name: &str) -> bool {
    // Reject protocol-relative URLs (security risk) - must check before relative URLs
    if url.starts_with("//") {
        return false;
    }

    // Reject backslashes: browsers normalize `/\evil.com` to `//evil.com`,
    // turning a "relative" path into a protocol-relative external redirect.
    if url.contains('\\') {
        return false;
    }

    // Only allow relative URLs (but not protocol-relative)
    url.starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_redirect_relative_url() {
        assert!(is_valid_redirect("/dashboard", "localhost"));
        assert!(is_valid_redirect("/api/v1/auth", "example.com"));
        assert!(is_valid_redirect("/", "localhost"));
    }

    #[test]
    fn test_is_valid_redirect_rejects_absolute_urls() {
        // Absolute URLs are no longer allowed
        assert!(!is_valid_redirect(
            "http://localhost/dashboard",
            "localhost"
        ));
        assert!(!is_valid_redirect("https://example.com/api", "example.com"));
    }

    #[test]
    fn test_is_valid_redirect_rejects_protocol_relative() {
        // Protocol-relative URLs are not allowed
        assert!(!is_valid_redirect("//evil.com", "localhost"));
        assert!(!is_valid_redirect("//example.com/path", "example.com"));
    }

    #[test]
    fn test_is_valid_redirect_rejects_backslashes() {
        // Backslashes are normalized to forward slashes by browsers, so
        // `/\evil.com` would become the protocol-relative `//evil.com`.
        assert!(!is_valid_redirect("/\\evil.com", "localhost"));
        assert!(!is_valid_redirect("\\evil.com", "localhost"));
        assert!(!is_valid_redirect("/path\\with-backslash", "localhost"));
    }

    #[test]
    fn test_is_valid_redirect_rejects_invalid_protocols() {
        assert!(!is_valid_redirect("ftp://localhost/file", "localhost"));
        assert!(!is_valid_redirect("javascript:alert(1)", "localhost"));
    }

    #[test]
    fn test_is_valid_redirect_empty_string() {
        assert!(!is_valid_redirect("", "localhost"));
    }

    #[test]
    fn test_is_valid_redirect_relative_with_special_chars() {
        assert!(is_valid_redirect("/path/with-dash", "localhost"));
        assert!(is_valid_redirect("/path/with_underscore", "localhost"));
        assert!(is_valid_redirect("/path/with.dot", "localhost"));
    }

    #[test]
    fn test_is_valid_redirect_relative_with_encoded_chars() {
        assert!(is_valid_redirect("/path%20with%20spaces", "localhost"));
        assert!(is_valid_redirect(
            "/path?query=value%20encoded",
            "localhost"
        ));
    }

    #[test]
    fn test_callback_query_missing_code() {
        // `code` is optional so providers' `?error=...` redirects deserialize
        let json = r#"{"state": "test_state"}"#;
        let query: CallbackQuery = serde_json::from_str(json).unwrap();
        assert!(query.code.is_none());
        assert_eq!(query.state.as_deref(), Some("test_state"));
    }

    #[test]
    fn test_callback_query_missing_state() {
        let json = r#"{"code": "test_code"}"#;
        let query: CallbackQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.code.as_deref(), Some("test_code"));
        assert!(query.state.is_none());
    }

    #[test]
    fn test_callback_query_error_params() {
        let json =
            r#"{"error": "access_denied", "error_description": "The user has denied access"}"#;
        let query: CallbackQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.error.as_deref(), Some("access_denied"));
        assert_eq!(
            query.error_description.as_deref(),
            Some("The user has denied access")
        );
        assert!(query.code.is_none());
        assert!(query.state.is_none());
    }

    #[test]
    fn test_is_valid_redirect_data_url() {
        assert!(!is_valid_redirect(
            "data:text/html,<script>alert(1)</script>",
            "localhost"
        ));
    }

    #[test]
    fn test_is_valid_redirect_file_url() {
        assert!(!is_valid_redirect("file:///etc/passwd", "localhost"));
    }
}

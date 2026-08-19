//! Authentication extractors and middleware
//!
//! Provides convenient extractors for getting the authenticated user from sessions
//! and middleware for requiring authentication on routes.

use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};

use crate::app::AppState;
use crate::middleware::api_token::ApiTokenAuth;
use crate::middleware::SessionExtension;
use crate::models::{ApiToken, User};
use crate::util::auth::Authentication;
use crate::util::errors::{forbidden, unauthorized, BoxedAppError};
use crate::util::HashedToken;
use std::sync::Arc;
use subtle::ConstantTimeEq;

/// Authenticated user ID extractor
///
/// Extracts the currently authenticated user's ID from the request extensions.
/// The global `authenticate` middleware populates this for both cookie sessions
/// and API tokens. Returns a 401 Unauthorized error if the user is not logged in.
///
/// # Example
///
/// ```ignore
/// pub async fn dashboard(
///     CurrentUserId(user_id): CurrentUserId,
///     State(state): State<AppState>,
/// ) -> AppResult<HtmlTemplate<DashboardTemplate>> {
///     let user = User::filter(User::fields().id().eq(user_id))
///         .first()
///         .exec(&mut state.0.database.db_clone())
///         .await?;
///     Ok(HtmlTemplate::new(DashboardTemplate { user }))
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CurrentUserId(pub u64);

impl<S: Send + Sync> FromRequestParts<S> for CurrentUserId {
    type Rejection = BoxedAppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        if let Some(id) = parts.extensions.get::<CurrentUserId>() {
            return Ok(*id);
        }

        if let Some(auth) = parts.extensions.get::<Authentication>() {
            return Ok(CurrentUserId(auth.user_id()));
        }

        let session = parts
            .extensions
            .get::<SessionExtension>()
            .ok_or_else(|| unauthorized("Session not found"))?;

        let user_id = session
            .get("user_id")
            .ok_or_else(|| unauthorized("Not logged in"))?;

        let user_id = user_id
            .parse::<u64>()
            .map_err(|_| unauthorized("Invalid session"))?;

        Ok(CurrentUserId(user_id))
    }
}

/// Optional authenticated user ID extractor
///
/// Extracts the currently authenticated user's ID from the request extensions if present.
/// Returns None if the user is not logged in.
///
/// # Example
///
/// ```ignore
/// pub async fn public_page(
///     OptionalCurrentUserId(user_id): OptionalCurrentUserId,
/// ) -> HtmlTemplate<PublicTemplate> {
///     match user_id {
///         Some(id) => HtmlTemplate::new(PublicTemplate { user_id: Some(id) }),
///         None => HtmlTemplate::new(PublicTemplate { user_id: None }),
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct OptionalCurrentUserId(pub Option<u64>);

impl<S: Send + Sync> FromRequestParts<S> for OptionalCurrentUserId {
    type Rejection = BoxedAppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        if let Some(id) = parts.extensions.get::<CurrentUserId>() {
            return Ok(OptionalCurrentUserId(Some(id.0)));
        }

        if let Some(auth) = parts.extensions.get::<Authentication>() {
            return Ok(OptionalCurrentUserId(Some(auth.user_id())));
        }

        let session = parts.extensions.get::<SessionExtension>();

        let user_id = match session.and_then(|s| s.get("user_id")) {
            Some(id) => id.parse::<u64>().ok(),
            None => None,
        };

        Ok(OptionalCurrentUserId(user_id))
    }
}

/// Global authentication middleware
///
/// Populates request extensions with the current user's authentication context.
/// It first checks for a valid API token in the `Authorization` header. If present
/// and valid, it sets `Authentication` and `CurrentUserId` and does not require
/// a session. If no token is provided, it falls back to the session cookie.
pub async fn authenticate(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    // If another middleware has already set the user, continue
    if req.extensions().get::<CurrentUserId>().is_some() {
        return next.run(req).await;
    }

    // Metrics endpoints authenticate with their own bearer token and should
    // not be validated against API tokens.
    let path = req.uri().path().trim_end_matches('/');
    if path == "/metrics" || path == "/api/private/metrics" {
        return next.run(req).await;
    }

    // Try session first so we have the auth context available for route extractors.
    // Load the user and reject the request if the account is currently locked.
    if let Some(user_id) = req
        .extensions()
        .get::<SessionExtension>()
        .and_then(|s| s.get("user_id"))
        .and_then(|s| s.parse::<u64>().ok())
    {
        let mut db = state.0.database.db_clone();
        match User::get_by_id(&mut db, user_id).await {
            Ok(user) if user.is_locked() => {
                let reason = user
                    .account_lock_reason
                    .unwrap_or_else(|| "Account is locked".into());
                return forbidden(reason).into_response();
            }
            Ok(_) => {
                req.extensions_mut().insert(CurrentUserId(user_id));
                req.extensions_mut()
                    .insert(Authentication::Cookie { user_id });
            }
            Err(_) => {
                // Invalid or stale session user; treat as not authenticated.
            }
        }
    }

    // If an Authorization header is present, prefer token auth
    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token_str) = auth_header.strip_prefix("Bearer ") {
            match validate_token(&state, token_str).await {
                Ok(auth) => {
                    let user_id = auth.user_id;
                    req.extensions_mut().insert(auth.clone());
                    req.extensions_mut().insert(CurrentUserId(user_id));
                    req.extensions_mut().insert(Authentication::Token {
                        user_id,
                        token_id: auth.token_id,
                        api_token: auth.api_token.clone(),
                    });
                }
                Err(response) => return response,
            }
        }
    }

    next.run(req).await
}

async fn validate_token(state: &AppState, token_str: &str) -> Result<ApiTokenAuth, Response> {
    let hashed_token =
        HashedToken::parse(token_str).map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;

    let mut db = state.0.database.db_clone();

    let mut api_token = ApiToken::filter(
        ApiToken::fields()
            .token()
            .eq(hashed_token.as_bytes().to_vec()),
    )
    .first()
    .exec(&mut db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    .ok_or(StatusCode::UNAUTHORIZED.into_response())?;

    let stored_hash: &[u8] = api_token.token.as_slice();
    let provided_hash: &[u8] = hashed_token.as_bytes();
    if !bool::from(stored_hash.ct_eq(provided_hash)) {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    }

    if api_token.revoked || !api_token.is_valid() {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    }

    // Verify the owning user is not locked before treating the token as valid.
    let user = User::get_by_id(&mut db, api_token.user_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;
    if user.is_locked() {
        return Err(StatusCode::FORBIDDEN.into_response());
    }

    // Update last_used_at timestamp
    let last_used_at = Some(jiff::Timestamp::now());
    toasty::update!(api_token { last_used_at })
        .exec(&mut db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(ApiTokenAuth {
        user_id: api_token.user_id,
        token_id: api_token.id,
        api_token: Arc::new(api_token),
    })
}

/// Require authenticated user middleware
///
/// Returns a 401 Unauthorized error if the request is not authenticated.
/// Works with both cookie sessions (set by the session middleware) and
/// API tokens (validated by the global `authenticate` middleware).
///
/// # Example
///
/// ```ignore
/// let router = Router::new()
///     .route("/api/dashboard", get(dashboard_handler))
///     .route_layer(middleware::from_fn(require_auth));
/// ```
pub async fn require_auth(req: Request, next: Next) -> Response {
    let is_authenticated = req.extensions().get::<CurrentUserId>().is_some()
        || req.extensions().get::<Authentication>().is_some()
        || req
            .extensions()
            .get::<SessionExtension>()
            .and_then(|s| s.get("user_id"))
            .is_some();

    if !is_authenticated {
        return unauthorized("Not logged in").response();
    }

    next.run(req).await
}

/// Require login middleware
///
/// Redirects to the GitHub OAuth login page if the user is not authenticated.
/// Use this for routes that require authentication but should redirect to login
/// instead of returning a 401 error.
///
/// # Example
///
/// ```ignore
/// let router = Router::new()
///     .route("/dashboard", get(dashboard_handler))
///     .route_layer(middleware::from_fn_with_state(
///         app_state.clone(),
///         require_login
///     ));
/// ```
pub async fn require_login(State(_state): State<AppState>, req: Request, next: Next) -> Response {
    let is_authenticated = req.extensions().get::<CurrentUserId>().is_some()
        || req.extensions().get::<Authentication>().is_some()
        || req
            .extensions()
            .get::<SessionExtension>()
            .and_then(|s| s.get("user_id"))
            .is_some();

    if !is_authenticated {
        // Redirect to GitHub OAuth login
        let redirect_url = format!(
            "/api/v1/auth/github/authorize?redirect_to={}",
            req.uri().path()
        );
        return Redirect::to(&redirect_url).into_response();
    }

    next.run(req).await
}

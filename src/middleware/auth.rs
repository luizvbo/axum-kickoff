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
///
/// This middleware is deliberately lazy for cookie sessions: the session
/// middleware already decoded the signed cookie, so the `user_id` claim is
/// trusted without a database lookup. The user record (and account-lock check)
/// is only loaded by `require_auth`/`require_login` on routes that actually
/// need authentication — mirroring crates.io's `AuthCheck` pattern. Bearer
/// tokens are still validated eagerly because the token lookup and the
/// `last_used_at` update are inherent to token authentication.
pub async fn authenticate(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    // If another middleware has already set the user, continue
    if req.extensions().get::<CurrentUserId>().is_some() {
        return next.run(req).await;
    }

    // Cookie sessions: trust the session extension populated by the session
    // middleware. Whether the user still exists, is active, and is not locked
    // is enforced lazily on authentication-required routes — see
    // `enforce_authentication`.
    if let Some(user_id) = req
        .extensions()
        .get::<SessionExtension>()
        .and_then(|s| s.get("user_id"))
        .and_then(|s| s.parse::<u64>().ok())
    {
        req.extensions_mut().insert(CurrentUserId(user_id));
        req.extensions_mut()
            .insert(Authentication::Cookie { user_id });
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

    // Verify the owning user is active and not locked before treating the
    // token as valid.
    let user = User::get_by_id(&mut db, api_token.user_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;
    if !user.is_active || user.is_locked() {
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

/// Enforce authentication at the point where a route requires it.
///
/// Token-authenticated requests were already fully validated — including the
/// account-active and account-lock checks — by `authenticate`, so they pass
/// through untouched. Cookie sessions only carry a `user_id` claim, so the
/// `User` record is loaded here, exactly once and only for requests that need
/// it. Deactivated and locked users are rejected with 403; sessions
/// referencing missing users get 401.
///
/// On success the loaded `User` is inserted into the request extensions so
/// handlers can reuse it without a second lookup.
async fn enforce_authentication(state: &AppState, req: &mut Request) -> Result<(), Response> {
    if req
        .extensions()
        .get::<Authentication>()
        .is_some_and(|auth| auth.is_token())
    {
        return Ok(());
    }

    let user_id = req
        .extensions()
        .get::<CurrentUserId>()
        .map(|id| id.0)
        .or_else(|| {
            req.extensions()
                .get::<SessionExtension>()
                .and_then(|s| s.get("user_id"))
                .and_then(|s| s.parse::<u64>().ok())
        });

    let Some(user_id) = user_id else {
        return Err(unauthorized("Not logged in").response());
    };

    let mut db = state.0.database.db_clone();
    match User::get_by_id(&mut db, user_id).await {
        Ok(user) if !user.is_active => Err(forbidden("Account is not active").response()),
        Ok(user) if user.is_locked() => {
            let reason = user
                .account_lock_reason
                .clone()
                .unwrap_or_else(|| "Account is locked".into());
            Err(forbidden(reason).response())
        }
        Ok(user) => {
            req.extensions_mut().insert(user);
            Ok(())
        }
        // The session references a user that no longer exists.
        Err(_) => Err(unauthorized("Not logged in").response()),
    }
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
///     .route_layer(middleware::from_fn_with_state(
///         app_state.clone(),
///         require_auth
///     ));
/// ```
pub async fn require_auth(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    match enforce_authentication(&state, &mut req).await {
        Ok(()) => next.run(req).await,
        Err(response) => response,
    }
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
pub async fn require_login(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    match enforce_authentication(&state, &mut req).await {
        Ok(()) => next.run(req).await,
        Err(response) => {
            if response.status() == StatusCode::UNAUTHORIZED {
                let redirect_url = format!(
                    "/api/v1/auth/github/authorize?redirect_to={}",
                    req.uri().path()
                );
                return Redirect::to(&redirect_url).into_response();
            }
            response
        }
    }
}

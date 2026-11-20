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
/// Populates request extensions with the current user's authentication
/// context from the session cookie. This middleware is deliberately lazy:
/// the session middleware already decoded the signed cookie, so the
/// `user_id` claim is trusted without a database lookup. The user record
/// (and account-lock check) is only loaded by `require_auth`/`require_login`
/// on routes that actually need authentication — mirroring crates.io's
/// `AuthCheck` pattern.
///
/// `Authorization: Bearer` headers are intentionally ignored here — token
/// validation only runs inside `enforce_authentication` on protected routes,
/// so a stale or forged token cannot 401 a public page. A consequence of
/// that split, matching crates.io's semantics: public-but-optionally-auth
/// routes that read `Authentication`/`OptionalCurrentUserId` without
/// `require_auth` see cookie sessions only; Bearer tokens on them are
/// ignored entirely.
pub async fn authenticate(
    State(_state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
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

    next.run(req).await
}

async fn validate_token(state: &AppState, token_str: &str) -> Result<Authentication, Response> {
    let hashed_token =
        HashedToken::parse(token_str).map_err(|_| unauthorized("Invalid API token").response())?;

    let mut db = state.0.database.db_clone();

    let mut api_token = ApiToken::filter(
        ApiToken::fields()
            .token()
            .eq(hashed_token.as_bytes().to_vec()),
    )
    .first()
    .exec(&mut db)
    .await
    .map_err(|e| crate::util::errors::db_error(e).response())?
    .ok_or_else(|| unauthorized("Invalid API token").response())?;

    let stored_hash: &[u8] = api_token.token.as_slice();
    let provided_hash: &[u8] = hashed_token.as_bytes();
    if !bool::from(stored_hash.ct_eq(provided_hash)) {
        return Err(unauthorized("Invalid API token").response());
    }

    if api_token.revoked || !api_token.is_valid() {
        return Err(unauthorized("Invalid API token").response());
    }

    // Verify the owning user is active and not locked before treating the
    // token as valid.
    let user = User::get_by_id(&mut db, api_token.user_id)
        .await
        .map_err(|_| unauthorized("Invalid API token").response())?;
    if !user.is_active || user.is_locked() {
        return Err(forbidden("Account is not active").response());
    }

    // Refresh last_used_at at most once per hour. A write on every
    // bearer-token request would serialize token-authenticated traffic onto
    // this row; a stale timestamp is not worth failing the request over.
    let now = jiff::Timestamp::now();
    let stale = api_token
        .last_used_at
        .is_none_or(|ts| ts < now - jiff::SignedDuration::from_hours(1));
    if stale {
        let last_used_at = Some(now);
        if let Err(e) = toasty::update!(api_token { last_used_at })
            .exec(&mut db)
            .await
        {
            tracing::warn!(error = %e, "Failed to update API token last_used_at");
        }
    }

    Ok(Authentication::Token {
        user_id: api_token.user_id,
        token_id: api_token.id,
        api_token: Arc::new(api_token),
    })
}

/// Enforce authentication at the point where a route requires it.
///
/// An `Authorization: Bearer` header wins: the token is validated and the
/// request authenticates as its owner — an invalid token is a 401 even when
/// a valid session cookie is also present, matching crates.io. The scheme
/// is case-insensitive per RFC 6750 and the token may carry surrounding
/// whitespace.
///
/// Without a Bearer header, the cookie-session `user_id` claim loads the
/// `User` record here, exactly once and only for requests that need it.
/// Sessions stamped with a stale `session_epoch` (see
/// [`User::bump_session_epoch`]) get 401, as do sessions referencing missing
/// users; deactivated and locked users get 403.
///
/// On success the loaded `User` is inserted into the request extensions so
/// handlers can reuse it without a second lookup.
async fn enforce_authentication(state: &AppState, req: &mut Request) -> Result<(), Response> {
    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        let (scheme, token_str) = auth_header.split_once(' ').unwrap_or(("", auth_header));
        if scheme.eq_ignore_ascii_case("bearer") {
            let auth = validate_token(state, token_str.trim()).await?;
            req.extensions_mut().insert(CurrentUserId(auth.user_id()));
            req.extensions_mut().insert(auth);
            return Ok(());
        }
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
            // Per-user session revocation: the cookie must carry the user's
            // current epoch. A missing stamp (pre-epoch sessions) or a
            // mismatch after `bump_session_epoch` ends the session.
            let session_epoch = req
                .extensions()
                .get::<SessionExtension>()
                .and_then(|s| s.get(crate::middleware::session::SESSION_EPOCH))
                .and_then(|v| v.parse::<i32>().ok());

            if session_epoch != Some(user.session_epoch) {
                return Err(unauthorized("Session expired").response());
            }

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
/// Redirects to the login page if the user is not authenticated.
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
                let redirect_url = format!("/login?redirect_to={}", req.uri().path());
                return Redirect::to(&redirect_url).into_response();
            }
            response
        }
    }
}

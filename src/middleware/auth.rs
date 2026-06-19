//! Authentication extractors and middleware
//!
//! Provides convenient extractors for getting the authenticated user from sessions
//! and middleware for requiring authentication on routes.

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};

use crate::app::AppState;
use crate::middleware::SessionExtension;
use crate::util::errors::{unauthorized, BoxedAppError};

/// Authenticated user ID extractor
///
/// Extracts the currently authenticated user's ID from the session.
/// Returns a 401 Unauthorized error if the user is not logged in.
///
/// # Example
///
/// ```rust,no_run
/// pub async fn dashboard(
///     CurrentUserId(user_id): CurrentUserId,
///     State(state): State<AppState>,
/// ) -> AppResult<HtmlTemplate<DashboardTemplate>> {
///     let user = User::filter(User::fields().id().eq(user_id))
///         .first()
///         .exec(&mut state.0.database.db_clone())
///         .await?;
///     Ok(HtmlTemplate(DashboardTemplate { user }))
/// }
/// ```
pub struct CurrentUserId(pub u64);

impl<S: Send + Sync> FromRequestParts<S> for CurrentUserId {
    type Rejection = BoxedAppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
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
/// Extracts the currently authenticated user's ID from the session if present.
/// Returns None if the user is not logged in.
///
/// # Example
///
/// ```rust,no_run
/// pub async fn public_page(
///     OptionalCurrentUserId(user_id): OptionalCurrentUserId,
/// ) -> HtmlTemplate<PublicTemplate> {
///     match user_id {
///         Some(id) => HtmlTemplate(PublicTemplate { user_id: Some(id) }),
///         None => HtmlTemplate(PublicTemplate { user_id: None }),
///     }
/// }
/// ```
pub struct OptionalCurrentUserId(pub Option<u64>);

impl<S: Send + Sync> FromRequestParts<S> for OptionalCurrentUserId {
    type Rejection = BoxedAppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let session = parts.extensions.get::<SessionExtension>();

        let user_id = match session.and_then(|s| s.get("user_id")) {
            Some(id) => id.parse::<u64>().ok(),
            None => None,
        };

        Ok(OptionalCurrentUserId(user_id))
    }
}

/// Require session user middleware
///
/// Returns a 401 Unauthorized error if the user is not authenticated via session.
/// Use this for API routes that require authentication and should return 401
/// instead of redirecting to login.
///
/// # Example
///
/// ```rust,no_run
/// let router = Router::new()
///     .route("/api/dashboard", get(dashboard_handler))
///     .route_layer(middleware::from_fn(require_session_user));
/// ```
pub async fn require_session_user(req: Request, next: Next) -> Response {
    let session = req.extensions().get::<SessionExtension>();

    let user_id = session.and_then(|s| s.get("user_id"));

    if user_id.is_none() {
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
/// ```rust,no_run
/// let router = Router::new()
///     .route("/dashboard", get(dashboard_handler))
///     .route_layer(middleware::from_fn_with_state(
///         app_state.clone(),
///         require_login
///     ));
/// ```
pub async fn require_login(State(_state): State<AppState>, req: Request, next: Next) -> Response {
    let session = req.extensions().get::<SessionExtension>();

    let user_id = session.and_then(|s| s.get("user_id"));

    if user_id.is_none() {
        // Redirect to GitHub OAuth login
        let redirect_url = format!(
            "/api/v1/auth/github/authorize?redirect_to={}",
            req.uri().path()
        );
        return Redirect::to(&redirect_url).into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_user_id_struct() {
        // This is a compile-time test to ensure the struct exists
        // Actual functionality tests would require a full test app setup
        let _ = std::marker::PhantomData::<CurrentUserId>;
    }

    #[test]
    fn test_optional_current_user_id_struct() {
        let _ = std::marker::PhantomData::<OptionalCurrentUserId>;
    }
}

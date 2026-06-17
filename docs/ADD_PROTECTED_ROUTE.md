# Add a Protected Route

This guide shows you how to add a route that requires user authentication.

## Overview

Protected routes require users to be logged in. For browser requests, anonymous users should be redirected to the GitHub login page. For API requests, return a `401 Unauthorized` response.

## Step 1: Create a CurrentUser Extractor

First, create a reusable extractor for authenticated users in `src/middleware/auth.rs`:

```rust
use axum::{
    async_trait,
    extract::{FromRequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::request::FromRef,
    RequestPartsExt,
};
use crate::app::AppState;
use crate::middleware::SessionExtension;
use crate::models::User;
use crate::util::errors::{unauthorized, AppResult};

/// Extractor for authenticated users
///
/// Returns 401 if user is not logged in
pub struct CurrentUser(pub User);

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppResult<axum::http::StatusCode>;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        // Try session auth first
        if let Some(session) = parts.extract::<SessionExtension>().await.ok() {
            if let Some(user_id_str) = session.get("user_id") {
                if let Ok(user_id) = user_id_str.parse::<u64>() {
                    let mut db = state.0.database.db_clone();
                    if let Ok(user) = User::get_by_id(&mut db, user_id).await {
                        return Ok(CurrentUser(user));
                    }
                }
            }
        }

        // Try API token auth
        if let Ok(TypedHeader(auth)) = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state).await {
            let token = auth.token();
            // Validate token and fetch user
            // (Implement token validation logic)
        }

        Err(unauthorized("Authentication required"))
    }
}

/// Optional extractor for authenticated users
///
/// Returns None if user is not logged in
pub struct OptionalCurrentUser(pub Option<User>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalCurrentUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match CurrentUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalCurrentUser(Some(user.0))),
            Err(_) => Ok(OptionalCurrentUser(None)),
        }
    }
}
```

## Step 2: Use the Extractor in Your Handler

Use the `CurrentUser` extractor in your protected route handler:

```rust
use axum::extract::State;
use crate::middleware::auth::CurrentUser;
use crate::app::AppState;
use crate::util::errors::AppResult;
use crate::templates::HtmlTemplate;

pub async fn dashboard(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<DashboardTemplate>> {
    // User is authenticated, proceed with the handler
    let mut db = state.0.database.db_clone();

    // Fetch user-specific data
    let posts = Post::filter(Post::fields().user_id().eq(user.id))
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(HtmlTemplate(DashboardTemplate {
        user,
        posts,
    }))
}
```

## Step 3: Add the Route

Add the protected route to your router:

```rust
use axum::routing::get;
use crate::controllers::your_controller;

// In your router function
.route("/dashboard", get(controllers::your_controller::dashboard))
```

## Step 4: Handle Redirects for Browser Requests

For browser requests, you may want to redirect anonymous users to the login page instead of returning 401. Create a middleware or modify your extractor:

```rust
pub async fn require_auth_or_redirect(
    session: SessionExtension,
) -> Result<CurrentUser, Redirect> {
    if let Some(user_id_str) = session.get("user_id") {
        if let Ok(user_id) = user_id_str.parse::<u64>() {
            // Fetch user from database
            // Return CurrentUser
        }
    }

    // Redirect to login with return URL
    Err(Redirect::to("/api/v1/auth/github/authorize?redirect_to=/dashboard"))
}
```

## Complete Example

Here's a complete example for a protected dashboard page:

### Extractor (`src/middleware/auth.rs`)

```rust
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::FromRef,
};
use crate::app::AppState;
use crate::middleware::SessionExtension;
use crate::models::User;
use crate::util::errors::{unauthorized, AppResult};

pub struct CurrentUser(pub User);

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppResult<axum::http::StatusCode>;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        if let Some(session) = parts.extract::<SessionExtension>().await.ok() {
            if let Some(user_id_str) = session.get("user_id") {
                if let Ok(user_id) = user_id_str.parse::<u64>() {
                    let mut db = state.0.database.db_clone();
                    if let Ok(user) = User::get_by_id(&mut db, user_id).await {
                        return Ok(CurrentUser(user));
                    }
                }
            }
        }

        Err(unauthorized("Authentication required"))
    }
}
```

### Controller (`src/controllers/dashboard.rs`)

```rust
use axum::extract::State;
use crate::middleware::auth::CurrentUser;
use crate::app::AppState;
use crate::models::Post;
use crate::util::errors::{server_error, AppResult};
use crate::templates::HtmlTemplate;

pub async fn dashboard(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<DashboardTemplate>> {
    let mut db = state.0.database.db_clone();

    let posts = Post::filter(Post::fields().user_id().eq(user.id))
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(HtmlTemplate(DashboardTemplate {
        user: user.clone(),
        username: user.gh_login,
        post_count: posts.len(),
    }))
}
```

### Template (`templates/dashboard.html`)

```html
{% extends "base.html" %}

{% block title %}Dashboard{% endblock %}

{% block content %}
<div class="container mx-auto p-4">
    <h1 class="text-3xl font-bold mb-4">Welcome, {{ username }}!</h1>

    <div class="bg-white rounded-lg shadow p-6 mb-6">
        <h2 class="text-xl font-semibold mb-2">Your Posts</h2>
        <p class="text-gray-600">You have {{ post_count }} posts.</p>
    </div>
</div>
{% endblock %}
```

### Router (`src/router.rs`)

```rust
.route("/dashboard", get(controllers::dashboard::dashboard))
```

## API-Only Protected Routes

For API endpoints that should always return 401 (not redirect):

```rust
pub async fn api_create_post(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Json(req): Json<CreatePostRequest>,
) -> AppResult<impl IntoResponse> {
    // Handler logic
    Ok(Json(created_post))
}
```

## Optional Authentication

For routes that work with or without authentication:

```rust
use crate::middleware::auth::OptionalCurrentUser;

pub async fn public_page(
    OptionalCurrentUser(maybe_user): OptionalCurrentUser,
) -> AppResult<HtmlTemplate<PublicTemplate>> {
    let username = maybe_user.map(|u| u.gh_login);

    Ok(HtmlTemplate(PublicTemplate {
        username,
    }))
}
```

## Testing Protected Routes

Test that protected routes reject unauthenticated requests:

```rust
#[tokio::test]
async fn test_dashboard_requires_auth() {
    let app = create_test_app().await;

    let response = app
        .oneshot(Request::builder()
            .uri("/dashboard")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
```

## Next Steps

- Learn how to [add an HTMX form](ADD_HTMX_FORM.md) for creating resources
- Learn about [API token authentication](../AUTHENTICATION.md)
- Review the [production checklist](PRODUCTION_CHECKLIST.md)

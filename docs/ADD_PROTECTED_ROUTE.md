# Add a Protected Route

This guide shows how to add a route that requires an authenticated user.

## Overview

The template already provides the authentication extractors and middleware:

- `CurrentUserId` — returns the authenticated user's ID, or `401` if not logged in.
- `OptionalCurrentUserId` — returns `Some(user_id)` or `None`.
- `Authentication` — returns an `Authentication::Cookie { user_id }` or `Authentication::Token { ... }`.
- `require_auth` middleware — returns `401 Unauthorized` for unauthenticated requests.
- `require_login` middleware — redirects browser requests to the GitHub OAuth login.

## Step 1: Add the route and protect it

In `src/router.rs`, add the route and a `route_layer` that requires authentication:

```rust
use axum::routing::{get, post};
use crate::controllers::dashboard;
use crate::middleware::{require_auth, require_login};

// API route that returns 401 when not authenticated
router
    .route("/api/v1/dashboard", get(dashboard::api_dashboard))
    .route_layer(axum::middleware::from_fn(require_auth));

// Browser route that redirects to login when not authenticated
router
    .route("/dashboard", get(dashboard::dashboard_page))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        require_login,
    ));
```

## Step 2: Use the extractor in the handler

`CurrentUserId` gives you the user's ID; load the `User` and any related data in the handler.

```rust
use axum::extract::State;
use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::{Post, User};
use crate::templates::HtmlTemplate;
use crate::util::errors::{server_error, AppResult};

pub async fn dashboard_page(
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<DashboardTemplate>> {
    let mut db = state.0.database.db_clone();

    let user = User::get_by_id(&mut db, user_id)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let posts = Post::filter(Post::fields().user_id().eq(user_id))
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(HtmlTemplate::new(DashboardTemplate { user, posts }))
}
```

If you need to know whether the request came from a session or an API token, use `Authentication`:

```rust
use crate::util::auth::Authentication;

pub async fn api_dashboard(
    auth: Authentication,
    State(state): State<AppState>,
) -> AppResult<Json<DashboardData>> {
    let mut db = state.0.database.db_clone();
    let user = User::get_by_id(&mut db, auth.user_id())
        .await
        .map_err(|e| server_error(e.to_string()))?;

    // auth.is_token() tells you if this is an API-token request
    Ok(Json(DashboardData { user }))
}
```

## Step 3: Optional authentication

For routes that work with or without a logged-in user, use `OptionalCurrentUserId`:

```rust
use crate::middleware::OptionalCurrentUserId;

pub async fn public_page(
    OptionalCurrentUserId(maybe_user): OptionalCurrentUserId,
) -> AppResult<HtmlTemplate<PublicTemplate>> {
    let username = maybe_user.map(|id| id.to_string());
    Ok(HtmlTemplate::new(PublicTemplate { username }))
}
```

## Step 4: Create the template

Create `templates/dashboard.html`:

```html
{% extends "base.html" %}

{% block title %}Dashboard{% endblock %}

{% block content %}
<div class="container mx-auto p-4">
    <h1 class="text-3xl font-bold mb-4">Welcome, {{ user.gh_login }}!</h1>
    <p>You have {{ posts.len() }} posts.</p>
</div>
{% endblock %}
```

Add the template struct in `src/templates/mod.rs` or wherever you keep template structs:

```rust
use askama::Template;
use crate::models::{Post, User};

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub user: User,
    pub posts: Vec<Post>,
}
```

## Testing

Test that an unauthenticated request is rejected and an authenticated request succeeds:

```rust
use axum_kickoff::tests::{TestApp, CookieUser, AnonymousUser};
use http::StatusCode;

#[tokio::test]
async fn test_dashboard_requires_auth() {
    let app = TestApp::new().await;

    // Anonymous users get 401
    let anon = AnonymousUser::new(app);
    let response = anon.get::<()>("/api/v1/dashboard").await;
    response.assert_status(StatusCode::UNAUTHORIZED);

    // Authenticated users succeed
    let mut db = app.db.db_clone();
    let user = app
        .user_builder("test_user")
        .build(&mut db)
        .await
        .expect("failed to create user");
    let session_key = app.state.session_key.clone();
    let user = CookieUser::new(app, user.id, session_key);
    let response = user.get::<()>("/api/v1/dashboard").await;
    response.assert_success();
}
```

## Next Steps

- Learn how to [add an HTMX form](ADD_HTMX_FORM.md) for creating resources
- Learn about [API token authentication](../AUTHENTICATION.md)
- Review the [production checklist](PRODUCTION_CHECKLIST.md)

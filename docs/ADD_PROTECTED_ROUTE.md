# Add a Protected Route

This guide shows you how to add a route that requires user authentication.

## Overview

The application ships the machinery already — you don't write extractors:

- **`authenticate`** middleware (in `src/middleware/auth.rs`) resolves the
  session cookie or an `Authorization: Bearer` API token and stores an
  `Authentication` + `CurrentUserId` in the request extensions.
- **`require_auth`** middleware returns `401` when no authentication is
  present — used as a `route_layer` on API route groups.
- **`require_login`** middleware redirects anonymous browser requests to
  `/login?redirect_to=<path>` — used for HTML pages.
- **`CurrentUserId(pub u64)`** — extractor: the authenticated user's ID or
  `401`.
- **`OptionalCurrentUserId(pub Option<u64>)`** — extractor: `Some(id)` or
  `None`, never rejects.

All are re-exported from `crate::middleware`.

## Step 1: Use the Extractor in Your Handler

```rust
use axum::extract::State;
use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::{Post, User};
use crate::router::{HtmlTemplate, PageContext};
use crate::util::errors::{db_error, AppResult};

pub async fn dashboard(
    ctx: PageContext,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<DashboardTemplate>> {
    let mut db = state.0.database.db_clone();

    let user = User::get_by_id(&mut db, user_id).await.map_err(db_error)?;
    let posts = Post::filter(Post::fields().user_id().eq(user_id))
        .exec(&mut db)
        .await
        .map_err(db_error)?;

    Ok(HtmlTemplate::new(DashboardTemplate {
        ctx,
        username: user.login,
        post_count: posts.len(),
    }))
}
```

## Step 2: Add the Route to the Right Group

Authentication requirements are enforced **per route group** via
`route_layer` in `src/router.rs` — put your route in the group matching its
needs:

- **API route** (401 on anonymous): add it to the protected API router that
  already runs `require_auth`, or add `.route_layer(from_fn_with_state(state.clone(), require_auth))`.
- **Browser page** (redirect to `/login`): use `require_login` the same way.
- **Public route**: the public router — no layer needed.

```rust
// API group — anonymous requests get 401
let protected_router = protected_router
    .route("/api/v1/things", get(list_things));

// Page route — anonymous requests redirect to /login
.route(
    "/dashboard",
    get(dashboard).route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        require_login,
    )),
)
```

## Optional Authentication

For routes that work with or without a logged-in user:

```rust
use crate::middleware::OptionalCurrentUserId;

pub async fn public_page(
    OptionalCurrentUserId(maybe_user_id): OptionalCurrentUserId,
) -> AppResult<HtmlTemplate<PublicTemplate>> {
    let username = match maybe_user_id {
        Some(id) => Some(load_user_login(id).await?),
        None => None,
    };
    // ...
}
```

## Scope Checks for API Tokens

`CurrentUserId` accepts either a session or a Bearer token. To restrict a
route to specific token scopes, take `Authentication` as an extractor and
run an `AuthCheck` — the same pattern `controllers/post.rs` and
`controllers/token.rs` use:

```rust
use crate::models::token::ActionScope;
use crate::util::auth::{AuthCheck, Authentication};

pub async fn create_thing(
    auth: Authentication,
    CurrentUserId(user_id): CurrentUserId,
    // ...
) -> AppResult<impl IntoResponse> {
    AuthCheck::new()
        .with_action_scope(ActionScope::Create)
        .for_crate("things")
        .check(&auth)?;
    // ...
}
```

A session-cookie user passes any `AuthCheck`; the check only constrains
Bearer tokens. See [api-token-scopes.md](api-token-scopes.md).

## Testing Protected Routes

```rust
#[tokio::test]
async fn test_dashboard_requires_auth() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/dashboard").await;
    // Browser pages redirect to /login; API routes return 401.
    assert!(matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::SEE_OTHER
    ));
}
```

See `src/tests/middleware_auth.rs` for cookie-user examples with CSRF.

## Next Steps

- Learn how to [add an HTMX form](ADD_HTMX_FORM.md) for creating resources
- Learn about [authentication](AUTHENTICATION.md)
- Review the [production checklist](PRODUCTION_CHECKLIST.md)

# Add an HTMX Form

This guide shows you how to add an HTMX form with CSRF protection, validation, and partial page updates — the same pattern the shipped `/examples/contact` route uses (`src/controllers/examples.rs`, `templates/examples/`).

## Overview

Everything below already exists; you only add the route, handler, and template:

- **CSRF validation** runs in middleware (`csrf_protect`/`protect` in
  `src/middleware/csrf.rs`) applied via `route_layer` — the handler never
  sees invalid submissions.
- **Token delivery**: `base.html` sets `hx-headers` on `<body>` with the
  `X-CSRF-Token` header (omitted for sessionless anonymous pages), so every
  HTMX request carries the token automatically. Classic forms can send the
  `csrf_token` hidden field instead.
- **The CSRF token itself** comes from `PageContext`/`get_or_create_csrf_token`.

## Step 1: Create the Form Template

Create `templates/create_post.html` — any template extending `base.html`
needs the `ctx` field (the layout renders `ctx.csrf_token`, `ctx.csp_nonce`,
`ctx.app_name`):

```html
{% extends "base.html" %}

{% block title %}Create Post{% endblock %}

{% block content %}
<div class="container mx-auto p-4" id="post-form-container">
    <h1 class="text-3xl font-bold mb-6">Create Post</h1>

    <form
        hx-post="/posts"
        hx-target="#post-form-container"
        hx-swap="outerHTML"
        class="max-w-2xl space-y-4"
    >
        <!-- HTMX sends ctx.csrf_token via the body hx-headers automatically;
             include the hidden field only for non-HTMX fallbacks: -->
        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">

        <div>
            <label for="title" class="block text-sm font-medium mb-2">Title</label>
            <input
                type="text"
                name="title"
                id="title"
                value="{{ title }}"
                class="w-full px-3 py-2 border rounded-lg"
                required
            >
            {% if errors.title %}
            <p class="text-red-500 text-sm mt-1">{{ errors.title }}</p>
            {% endif %}
        </div>

        <div>
            <label for="content" class="block text-sm font-medium mb-2">Content</label>
            <textarea
                name="content"
                id="content"
                rows="6"
                class="w-full px-3 py-2 border rounded-lg"
                required
            >{{ content }}</textarea>
            {% if errors.content %}
            <p class="text-red-500 text-sm mt-1">{{ errors.content }}</p>
            {% endif %}
        </div>

        <button
            type="submit"
            class="bg-blue-500 text-white px-4 py-2 rounded-lg hover:bg-blue-600"
        >
            Create Post
        </button>
    </form>
</div>
{% endblock %}
```

Template struct next to the handler:

```rust
use askama::Template;
use std::collections::HashMap;
use crate::router::PageContext;

#[derive(Template)]
#[template(path = "create_post.html")]
pub struct CreatePostTemplate {
    pub ctx: PageContext,
    pub title: String,
    pub content: String,
    pub errors: HashMap<String, String>,
}
```

## Step 2: Create the Handler

```rust
use axum::{extract::{Form, State}, response::IntoResponse};
use serde::Deserialize;
use std::collections::HashMap;
use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::Post;
use crate::router::{HtmlTemplate, PageContext};
use crate::util::errors::{db_error, AppResult};

#[derive(Deserialize)]
pub struct CreatePostForm {
    // The CSRF field is consumed by middleware; declaring it here just keeps
    // serde from complaining about unknown fields.
    #[serde(default)]
    pub csrf_token: String,
    pub title: String,
    pub content: String,
}

pub async fn new_post_form(ctx: PageContext) -> AppResult<HtmlTemplate<CreatePostTemplate>> {
    Ok(HtmlTemplate::new(CreatePostTemplate {
        ctx,
        title: String::new(),
        content: String::new(),
        errors: HashMap::new(),
    }))
}

pub async fn create_post(
    ctx: PageContext,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Form(form): Form<CreatePostForm>,
) -> AppResult<impl IntoResponse> {
    // No CSRF check here — the route-layer middleware already rejected
    // invalid tokens before this handler ran.
    let mut errors = HashMap::new();

    if form.title.trim().is_empty() {
        errors.insert("title", "Title is required".to_string());
    }
    if form.content.trim().is_empty() {
        errors.insert("content", "Content is required".to_string());
    }
    if form.title.len() > 200 {
        errors.insert("title", "Title must be less than 200 characters".to_string());
    }

    if !errors.is_empty() {
        // Re-render the same form as an HTMX partial — status 200 so htmx
        // swaps it in without triggering error handling.
        return Ok(HtmlTemplate::new(CreatePostTemplate {
            ctx,
            title: form.title,
            content: form.content,
            errors,
        }).into_response());
    }

    let mut db = state.0.database.db_clone();
    let post = toasty::create!(Post {
        user_id,
        title: form.title,
        content: form.content,
        published: false,
        created_at: jiff::Timestamp::now(),
        updated_at: jiff::Timestamp::now(),
    })
    .exec(&mut db)
    .await
    .map_err(db_error)?;

    // Success: return a fragment or redirect to the new post.
    Ok(axum::response::Html(format!(
        r#"<div class="bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded">
            Post created! <a href="/posts/{}" class="underline">View post</a>
        </div>"#,
        post.id
    )).into_response())
}
```

## Step 3: Add the Route

Register both routes in `src/router.rs`. Protect the POST with
`csrf_protect` (or `protect`) via `route_layer`, and `require_login` if only
signed-in users may submit — mirroring the `examples/contact` registration:

```rust
use axum::routing::{get, post};

.route("/posts/new", get(post::new_post_form))
.route(
    "/posts",
    post(post::create_post).route_layer(axum::middleware::from_fn(
        crate::middleware::csrf::csrf_protect,
    )),
)
```

`csrf_protect` validates the `X-CSRF-Token` header or the `csrf_token` form
field for unsafe methods; GET needs nothing.

## HTMX Attributes Reference

- `hx-post="/url"` - Send POST request to URL
- `hx-target="#element"` - Update specific element with response
- `hx-swap="outerHTML"` - Replace entire element (also: innerHTML, beforebegin, afterend, ...)
- `hx-indicator="#loading"` - Show loading indicator during request
- `hx-disabled-elt="button"` - Disable elements during request
- `hx-headers` - Set per-request headers (used by `base.html` for CSRF)

## Testing HTMX Forms

Unsafe methods require the session CSRF token — in tests, use
`CookieUser::init_csrf()` + `post_with_headers` (see `src/tests/posts.rs`):

```rust
let csrf_token = cookie_user.init_csrf().await;
let response = cookie_user
    .post_with_headers::<serde_json::Value>(
        "/posts",
        &form_body,
        cookie_user.headers_with_csrf(&csrf_token),
    )
    .await;
```

## Next Steps

- Learn how to [add a protected route](ADD_PROTECTED_ROUTE.md) for form access
- See the shipped example: `/examples/contact` (`src/controllers/examples.rs`)
- Learn about [CSRF protection](CSRF_PROTECTION.md)
- Review the [production checklist](PRODUCTION_CHECKLIST.md)

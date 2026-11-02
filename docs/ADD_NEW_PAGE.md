# Add a New Page

This guide shows you how to add a new page to your application.

## Overview

Adding a page involves four steps:
1. Create a route in the router
2. Create a handler function in a controller
3. Create an Askama template
4. Link to the page from your layout or navigation

## Step 1: Create the Route

Add a new route in `src/router.rs`:

```rust
use axum::routing::get;
use crate::controllers::your_controller;

// In the public_router (or the router group matching your auth needs)
.route("/about", get(your_controller::about_page))
```

## Step 2: Create the Handler

Create a handler function in `src/controllers/your_controller.rs` (or add to an existing controller):

```rust
use crate::router::{HtmlTemplate, PageContext};
use crate::util::errors::AppResult;

pub async fn about_page(
    ctx: PageContext,
) -> AppResult<HtmlTemplate<AboutTemplate>> {
    Ok(HtmlTemplate::new(AboutTemplate {
        ctx,
        description: "This is the about page.".to_string(),
    }))
}
```

`PageContext` is a request extractor provided by `src/router.rs`. It
supplies `app_name`, `csrf_token`, `csp_nonce`, and the enabled OAuth
providers — everything `base.html` renders.

## Step 3: Create the Template

Define the template struct next to the handler (there is no `src/templates/`
module — template structs live beside the code that renders them). Any
template that extends `base.html` **must** carry the `ctx` field, since the
base layout uses `ctx.csrf_token`, `ctx.csp_nonce`, and `ctx.app_name`:

```rust
use askama::Template;
use crate::router::PageContext;

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub ctx: PageContext,
    pub description: String,
}
```

Then create `templates/about.html`:

```html
{% extends "base.html" %}

{% block title %}About — {{ ctx.app_name }}{% endblock %}

{% block content %}
<div class="container mx-auto p-4">
    <h1 class="text-3xl font-bold mb-4">About</h1>
    <p class="text-gray-700">{{ description }}</p>
</div>
{% endblock %}
```

## Step 4: Link from Layout

Add a link to your new page in `templates/base.html` or your navigation component:

```html
<nav>
    <a href="/">Home</a>
    <a href="/about">About</a>
</nav>
```

## Complete Example

### Router (`src/router.rs`)

```rust
.route("/about", get(controllers::home::about))
```

### Controller (`src/controllers/home.rs`)

```rust
use askama::Template;
use crate::router::{HtmlTemplate, PageContext};
use crate::util::errors::AppResult;

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub ctx: PageContext,
    pub title: String,
    pub description: String,
}

pub async fn about(ctx: PageContext) -> AppResult<HtmlTemplate<AboutTemplate>> {
    Ok(HtmlTemplate::new(AboutTemplate {
        ctx,
        title: "About Us".to_string(),
        description: "This is the about page.".to_string(),
    }))
}
```

### Template (`templates/about.html`)

```html
{% extends "base.html" %}

{% block title %}{{ title }} — {{ ctx.app_name }}{% endblock %}

{% block content %}
<div class="container mx-auto p-4">
    <h1 class="text-3xl font-bold mb-4">{{ title }}</h1>
    <p class="text-gray-700">{{ description }}</p>
</div>
{% endblock %}
```

## Testing Your Page

Start the server and visit your new page:

```bash
cargo run -- server
```

Navigate to `http://localhost:8888/about` in your browser.

## Adding Dynamic Data

To pass dynamic data to your template, fetch it from the database in the
handler:

```rust
use axum::extract::State;
use crate::app::AppState;
use crate::router::{HtmlTemplate, PageContext};
use crate::util::errors::{db_error, AppResult};

pub async fn about_page(
    ctx: PageContext,
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<AboutTemplate>> {
    let mut db = state.0.database.db_clone();

    // Fetch data using Toasty
    let users = User::all().exec(&mut db).await
        .map_err(db_error)?;

    Ok(HtmlTemplate::new(AboutTemplate {
        ctx,
        user_count: users.len(),
    }))
}
```

Update your template to use the data:

```html
{% block content %}
<div class="container">
    <h1>About</h1>
    <p>We have {{ user_count }} users!</p>
</div>
{% endblock %}
```

## HTMX Partials

If a route returns both a full page and an HTMX-swapped fragment, use
`HtmlTemplate::new(template).with_partial(partial_template)` and render the
partial for `HX-Request` responses — see `controllers/examples.rs` and
`docs/HTMX_ASKAMA_PATTERNS.md` for the full pattern.

## Next Steps

- Learn how to [add a new model](ADD_NEW_MODEL.md)
- Learn how to [add a protected route](ADD_PROTECTED_ROUTE.md)
- Learn how to [add an HTMX form](ADD_HTMX_FORM.md)

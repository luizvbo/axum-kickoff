# Add a New Page

This guide shows you how to add a new page to your axum-kickoff application.

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

// In your router function
.route("/about", get(your_controller::about_page))
```

## Step 2: Create the Handler

Create a handler function in `src/controllers/your_controller.rs` (or add to an existing controller):

```rust
use axum::extract::State;
use crate::app::AppState;
use crate::util::errors::AppResult;
use crate::templates::HtmlTemplate;

pub async fn about_page(
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<AboutTemplate>> {
    // Fetch any data you need for the template
    let context = "Your data here";

    Ok(HtmlTemplate(AboutTemplate { context }))
}
```

## Step 3: Create the Template

Create a new template file in `templates/about.html`:

```html
{% extends "base.html" %}

{% block title %}About{% endblock %}

{% block content %}
<div class="container">
    <h1>About</h1>
    <p>{{ context }}</p>
</div>
{% endblock %}
```

Define the template struct in `src/templates/mod.rs`:

```rust
use askama::Template;

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub context: String,
}
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

Here's a complete example for a simple "About" page:

### Router (`src/router.rs`)

```rust
.route("/about", get(controllers::home::about))
```

### Controller (`src/controllers/home.rs`)

```rust
pub async fn about() -> AppResult<HtmlTemplate<AboutTemplate>> {
    Ok(HtmlTemplate(AboutTemplate {
        title: "About Us".to_string(),
        description: "This is the about page.".to_string(),
    }))
}
```

### Template (`templates/about.html`)

```html
{% extends "base.html" %}

{% block title %}About{% endblock %}

{% block content %}
<div class="container mx-auto p-4">
    <h1 class="text-3xl font-bold mb-4">{{ title }}</h1>
    <p class="text-gray-700">{{ description }}</p>
</div>
{% endblock %}
```

### Template Struct (`src/templates/mod.rs`)

```rust
#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub title: String,
    pub description: String,
}
```

## Testing Your Page

Start the server and visit your new page:

```bash
cargo run --bin server
```

Navigate to `http://localhost:8888/about` in your browser.

## Adding Dynamic Data

To pass dynamic data to your template, modify your handler to fetch data from the database or other sources:

```rust
pub async fn about_page(
    State(state): State<AppState>,
) -> AppResult<HtmlTemplate<AboutTemplate>> {
    let mut db = state.0.database.db_clone();

    // Fetch data using Toasty
    let users = User::all().exec(&mut db).await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(HtmlTemplate(AboutTemplate {
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

## Next Steps

- Learn how to [add a new model](ADD_NEW_MODEL.md)
- Learn how to [add a protected route](ADD_PROTECTED_ROUTE.md)
- Learn how to [add an HTMX form](ADD_HTMX_FORM.md)

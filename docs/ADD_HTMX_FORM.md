# Add an HTMX Form

This guide shows you how to add an HTMX form with CSRF protection, validation, and partial page updates.

## Overview

HTMX forms allow you to submit forms and update parts of the page without full page reloads. This guide covers:

- Adding CSRF protection
- Creating a form with HTMX attributes
- Handling form submission on the server
- Rendering validation errors
- Returning partial HTML updates

## Step 1: Add CSRF Protection

First, ensure CSRF middleware is set up. Create a CSRF token generator in `src/middleware/csrf.rs`:

```rust
use axum::{
    extract::{FromRef, FromRequestParts, State},
    http::request::Parts,
};
use crate::app::AppState;

pub struct CsrfToken(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for CsrfToken
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        
        // Generate or retrieve CSRF token from session
        // For now, generate a simple token
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let token = format!("csrf_{}", COUNTER.fetch_add(1, Ordering::SeqCst));
        
        Ok(CsrfToken(token))
    }
}
```

## Step 2: Create the Form Template

Create a form template with HTMX attributes in `templates/create_post.html`:

```html
{% extends "base.html" %}

{% block title %}Create Post{% endblock %}

{% block content %}
<div class="container mx-auto p-4">
    <h1 class="text-3xl font-bold mb-6">Create Post</h1>
    
    <form 
        hx-post="/posts"
        hx-target="#post-form"
        hx-swap="outerHTML"
        class="max-w-2xl"
    >
        <!-- CSRF Token -->
        <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
        
        <!-- Title Field -->
        <div class="mb-4">
            <label for="title" class="block text-sm font-medium mb-2">Title</label>
            <input 
                type="text" 
                name="title" 
                id="title"
                value="{{ title }}"
                class="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
                required
            >
            {% if errors.title %}
            <p class="text-red-500 text-sm mt-1">{{ errors.title }}</p>
            {% endif %}
        </div>
        
        <!-- Content Field -->
        <div class="mb-4">
            <label for="content" class="block text-sm font-medium mb-2">Content</label>
            <textarea 
                name="content" 
                id="content"
                rows="6"
                class="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
                required
            >{{ content }}</textarea>
            {% if errors.content %}
            <p class="text-red-500 text-sm mt-1">{{ errors.content }}</p>
            {% endif %}
        </div>
        
        <!-- Submit Button -->
        <button 
            type="submit"
            class="bg-blue-500 text-white px-4 py-2 rounded-lg hover:bg-blue-600"
        >
            Create Post
        </button>
    </form>
    
    <div id="post-form"></div>
</div>
{% endblock %}
```

## Step 3: Create the Handler

Create a handler that processes the form submission in `src/controllers/post.rs`:

```rust
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use crate::app::AppState;
use crate::middleware::auth::CurrentUser;
use crate::models::Post;
use crate::util::errors::{bad_request, server_error, AppResult};
use crate::templates::HtmlTemplate;

#[derive(Deserialize)]
pub struct CreatePostForm {
    pub csrf_token: String,
    pub title: String,
    pub content: String,
}

pub async fn create_post(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Form(form): Form<CreatePostForm>,
) -> AppResult<impl IntoResponse> {
    // Validate CSRF token
    // (Implement CSRF validation logic)
    
    // Validate form data
    let mut errors = std::collections::HashMap::new();
    
    if form.title.trim().is_empty() {
        errors.insert("title", "Title is required".to_string());
    }
    
    if form.content.trim().is_empty() {
        errors.insert("content", "Content is required".to_string());
    }
    
    if form.title.len() > 200 {
        errors.insert("title", "Title must be less than 200 characters".to_string());
    }
    
    // Return form with errors if validation fails
    if !errors.is_empty() {
        return Ok(HtmlTemplate(CreatePostTemplate {
            csrf_token: form.csrf_token,
            title: form.title,
            content: form.content,
            errors,
        }).into_response());
    }
    
    // Create the post
    let mut db = state.0.database.db_clone();
    
    let post = toasty::create!(Post {
        user_id: user.id,
        title: form.title,
        content: form.content,
        published: false,
        created_at: jiff::Timestamp::now(),
        updated_at: jiff::Timestamp::now(),
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;
    
    // Return success message or redirect
    Ok(Html(r#"
        <div class="bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded">
            Post created successfully!
        </div>
    "#).into_response())
}
```

## Step 4: Create the Success Template

Create a template for successful form submission:

```html
<div class="bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded mb-4">
    <p>Post created successfully!</p>
    <a href="/posts/{{ post_id }}" class="underline">View post</a>
</div>
```

## Step 5: Add the Route

Add the form and submission routes to your router:

```rust
use axum::routing::{get, post};
use crate::controllers::post;

.route("/posts/new", get(post::new_post_form))
.route("/posts", post(post::create_post))
```

## Complete Example

Here's a complete example for a post creation form:

### Template (`templates/create_post.html`)

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
        <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
        
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

### Controller (`src/controllers/post.rs`)

```rust
use axum::{extract::{Form, State}, response::Html};
use serde::Deserialize;
use std::collections::HashMap;
use crate::app::AppState;
use crate::middleware::auth::CurrentUser;
use crate::util::errors::AppResult;

#[derive(Deserialize)]
pub struct CreatePostForm {
    pub csrf_token: String,
    pub title: String,
    pub content: String,
}

pub async fn new_post_form(
    CsrfToken(csrf_token): CsrfToken,
) -> AppResult<HtmlTemplate<CreatePostTemplate>> {
    Ok(HtmlTemplate(CreatePostTemplate {
        csrf_token,
        title: String::new(),
        content: String::new(),
        errors: HashMap::new(),
    }))
}

pub async fn create_post(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Form(form): Form<CreatePostForm>,
) -> AppResult<Html<String>> {
    let mut errors = HashMap::new();
    
    if form.title.trim().is_empty() {
        errors.insert("title", "Title is required".to_string());
    }
    
    if form.content.trim().is_empty() {
        errors.insert("content", "Content is required".to_string());
    }
    
    if !errors.is_empty() {
        let template = CreatePostTemplate {
            csrf_token: form.csrf_token,
            title: form.title,
            content: form.content,
            errors,
        };
        return Ok(Html(template.render().unwrap()));
    }
    
    // Create post logic here...
    
    Ok(Html(r#"
        <div class="bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded">
            Post created successfully!
            <a href="/dashboard" class="underline ml-2">Return to dashboard</a>
        </div>
    "#.to_string()))
}
```

### Template Struct (`src/templates/mod.rs`)

```rust
#[derive(Template)]
#[template(path = "create_post.html")]
pub struct CreatePostTemplate {
    pub csrf_token: String,
    pub title: String,
    pub content: String,
    pub errors: HashMap<String, String>,
}
```

## HTMX Attributes Reference

Common HTMX attributes for forms:

- `hx-post="/url"` - Send POST request to URL
- `hx-target="#element"` - Update specific element with response
- `hx-swap="outerHTML"` - Replace entire element (options: innerHTML, outerHTML, beforebegin, afterend, etc.)
- `hx-indicator="#loading"` - Show loading indicator during request
- `hx-disabled-elt="button"` - Disable elements during request

## Loading Indicators

Add a loading indicator:

```html
<div id="loading" class="hidden">
    <div class="spinner">Loading...</div>
</div>

<form hx-post="/posts" hx-indicator="#loading">
    <!-- form fields -->
</form>
```

## Client-Side Validation

Add HTML5 validation attributes:

```html
<input 
    type="text" 
    name="title" 
    required
    minlength="3"
    maxlength="200"
    pattern="[a-zA-Z0-9\s]+"
>
```

## Testing HTMX Forms

Test form submission with curl:

```bash
curl -X POST http://localhost:8888/posts \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "csrf_token=token&title=Test&content=Content"
```

## Next Steps

- Learn how to [add a protected route](ADD_PROTECTED_ROUTE.md) for form access
- Learn about [CSRF protection best practices](../MIDDLEWARE.md)
- Review the [production checklist](PRODUCTION_CHECKLIST.md)

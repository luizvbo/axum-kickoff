# HTMX + Askama Patterns

This guide demonstrates practical patterns for building modern web applications using HTMX and Askama with Axum. These examples show how to build interactive, server-rendered applications without complex JavaScript frameworks.

## Table of Contents

- [Overview](#overview)
- [Why HTMX + Askama?](#why-htmx--askama)
- [Pattern 1: Full-Page Askama Templates](#pattern-1-full-page-askama-templates)
- [Pattern 2: Partial Template Responses](#pattern-2-partial-template-responses)
- [Pattern 3: HTMX Form Handling](#pattern-3-htmx-form-handling)
- [Pattern 4: Validation Error Handling](#pattern-4-validation-error-handling)
- [Pattern 5: Redirect vs HTML Partial Convention](#pattern-5-redirect-vs-html-partial-convention)
- [Pattern 6: JSON API Endpoints](#pattern-6-json-api-endpoints)
- [Live Examples](#live-examples)
- [Best Practices](#best-practices)

## Overview

The HTMX + Askama stack provides a simple, powerful approach to web development:

- **HTMX**: Adds AJAX capabilities to HTML without writing JavaScript
- **Askama**: Type-safe, compile-time templating for Rust
- **Axum**: Modern async web framework for Rust

This combination gives you:
- Server-side rendering with type safety
- Interactive UI without complex JavaScript
- Progressive enhancement (works without JS)
- Simple mental model (HTTP requests return HTML)

## Why HTMX + Askama?

Compared to traditional SPA frameworks (React, Vue, Svelte):

**Advantages:**
- Less JavaScript to write and maintain
- Better SEO (server-side rendered by default)
- Faster initial page load
- Simpler deployment (no build step for frontend)
- Type-safe templates with Askama
- Progressive enhancement works without JS

**Trade-offs:**
- Less sophisticated state management
- Fewer ecosystem tools
- Requires server round-trips for interactions

For most CRUD applications and content-heavy sites, HTMX + Askama is an excellent choice.

## Pattern 1: Full-Page Askama Templates

Full-page templates render complete HTML documents. Use these for initial page loads and navigation.

### Template Structure

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }}</title>
    <link rel="stylesheet" href="/static/css/style.css">
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <meta name="htmx-config" content='{"csrfToken": "{{ csrf_token }}"}'>
</head>
<body>
    <nav>
        <!-- Navigation -->
    </nav>

    <main>
        <!-- Page content -->
    </main>

    <footer>
        <!-- Footer -->
    </footer>
</body>
</html>
```

### Rust Handler

```rust
use askama::Template;
use axum::extract::Extension;
use crate::middleware::{get_or_create_csrf_token, SessionExtension};
use crate::router::HtmlTemplate;

#[derive(Template)]
#[template(path = "examples/contact.html")]
struct ContactPageTemplate {
    csrf_token: String,
}

pub async fn contact_page(Extension(session): Extension<SessionExtension>) -> impl IntoResponse {
    let csrf_token = get_or_create_csrf_token(&session);
    let template = ContactPageTemplate { csrf_token };
    HtmlTemplate(template)
}
```

### Route Definition

```rust
let public_router = Router::new()
    .route("/examples/contact", get(contact_page));
```

### Key Points

- **CSRF token**: Always include for forms that submit data
- **HTMX script**: Load HTMX from CDN or bundle it
- **htmx-config**: Configure CSRF token for automatic inclusion
- **Type safety**: Askama validates template variables at compile time

## Pattern 2: Partial Template Responses

Partial templates return HTML fragments, not full documents. HTMX swaps these fragments into the page.

### Partial Template

```html
<!-- templates/examples/contact_success.html -->
<div class="alert alert-success">
    <div class="alert-icon">✓</div>
    <div class="alert-content">
        <h4>Message Sent!</h4>
        <p>Thank you, {{ name }}. We've received your message.</p>
    </div>
</div>
```

### Rust Handler

```rust
#[derive(Template)]
#[template(path = "examples/contact_success.html")]
struct ContactSuccessTemplate {
    name: String,
}

pub async fn contact_submit(Form(form): Form<ContactForm>) -> Response {
    // ... validation and processing ...

    let template = ContactSuccessTemplate {
        name: form.name,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to render template: {}", err),
        ).into_response(),
    }
}
```

### HTMX Usage

```html
<form hx-post="/examples/contact" hx-target="#result" hx-swap="innerHTML">
    <!-- Form fields -->
</form>
<div id="result"></div>
```

### Key Points

- **No HTML boilerplate**: Partials contain only the fragment
- **hx-target**: Specifies where to insert the response
- **hx-swap**: Controls how the response is inserted (innerHTML, outerHTML, beforeend, etc.)
- **Error handling**: Always handle template render errors

## Pattern 3: HTMX Form Handling

HTMX forms submit via AJAX without page reloads, returning HTML partials for success or error states.

### Form with HTMX Attributes

```html
<form
    hx-post="/examples/contact"
    hx-target="#form-result"
    hx-swap="innerHTML"
    hx-indicator="#loading-indicator"
>
    <div class="form-group">
        <label for="name">Name *</label>
        <input type="text" id="name" name="name" required>
    </div>

    <div class="form-group">
        <label for="email">Email *</label>
        <input type="email" id="email" name="email" required>
    </div>

    <button type="submit">Send Message</button>

    <div id="loading-indicator" style="display: none;">
        Sending...
    </div>
</form>

<div id="form-result"></div>
```

### Handler with Validation

```rust
#[derive(Deserialize)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub message: String,
}

pub async fn contact_submit(Form(form): Form<ContactForm>) -> Response {
    // Validate
    let mut errors = Vec::new();

    if form.name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }

    if !errors.is_empty() {
        let template = ContactErrorsTemplate { errors };
        return match template.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => error_response(err),
        };
    }

    // Process form
    // ... save to database ...

    // Return success partial
    let template = ContactSuccessTemplate {
        name: form.name,
    };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => error_response(err),
    }
}
```

### Key HTMX Attributes

- **hx-post**: HTTP method and endpoint
- **hx-target**: Element to insert response into
- **hx-swap**: How to insert response (innerHTML, outerHTML, beforeend, afterend, etc.)
- **hx-indicator**: Element to show during request (adds htmx-request class)
- **hx-include**: Additional elements to include in request data

## Pattern 4: Validation Error Handling

Return validation errors as HTML partials that HTMX swaps into the page.

### Error Partial Template

```html
<!-- templates/examples/contact_errors.html -->
<div class="alert alert-error">
    <div class="alert-icon">⚠</div>
    <div class="alert-content">
        <h4>Please fix the following errors:</h4>
        <ul class="error-list">
            {% for error in errors %}
            <li class="error-item">{{ error }}</li>
            {% endfor %}
        </ul>
    </div>
</div>
```

### Handler with Error Handling

```rust
pub async fn contact_submit(Form(form): Form<ContactForm>) -> Response {
    let mut errors = Vec::new();

    // Validate each field
    if form.name.trim().is_empty() {
        errors.push("Name is required".to_string());
    } else if form.name.len() < 2 {
        errors.push("Name must be at least 2 characters".to_string());
    }

    if form.email.trim().is_empty() {
        errors.push("Email is required".to_string());
    } else if !form.email.contains('@') {
        errors.push("Email must be valid".to_string());
    }

    // Return errors if validation fails
    if !errors.is_empty() {
        let template = ContactErrorsTemplate { errors };
        return match template.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => error_response(err),
        };
    }

    // ... process valid form ...
}
```

### Key Points

- **Collect all errors**: Validate all fields before returning
- **Clear error messages**: Use user-friendly error text
- **Structured errors**: Use lists for multiple errors
- **Consistent styling**: Use alert/error CSS classes

## Pattern 5: Redirect vs HTML Partial Convention

Decide when to redirect vs return HTML partials based on the request type.

### Convention

| Request Type | Success Response | Error Response |
|-------------|-----------------|----------------|
| HTMX request | HTML partial | HTML partial |
| Traditional form | Redirect | HTML with errors |
| Navigation action | Redirect | Redirect or error page |

### Detecting HTMX Requests

HTMX adds the `HX-Request` header to all requests:

```rust
use axum::extract::HeaderMap;

pub async fn some_handler(headers: HeaderMap) -> Response {
    let is_htmx = headers.get("HX-Request").is_some();

    if is_htmx {
        // Return HTML partial
    } else {
        // Return full page or redirect
    }
}
```

### Redirect Example

```rust
use axum::response::Redirect;

pub async fn traditional_form_submit(Form(form): Form<ContactForm>) -> Response {
    // ... validate and process ...

    // Redirect on success (traditional form)
    Redirect::to("/examples/contact").into_response()
}
```

### HTMX Partial Example

```rust
pub async fn htmx_form_submit(Form(form): Form<ContactForm>) -> Response {
    // ... validate and process ...

    // Return HTML partial (HTMX)
    let template = ContactSuccessTemplate { name: form.name };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => error_response(err),
    }
}
```

### Key Points

- **HTMX requests**: Always return HTML partials (no redirects)
- **Traditional forms**: Redirect on success, show errors on same page
- **Navigation**: Use redirects for page navigation
- **API calls**: Return JSON for JavaScript consumers

## Pattern 6: JSON API Endpoints

For JavaScript consumers or external APIs, return JSON instead of HTML.

### JSON Response Struct

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ExampleJsonResponse {
    pub message: String,
    pub timestamp: String,
    pub data: HashMap<String, String>,
}
```

### JSON Handler

```rust
use axum::response::Json;
use chrono::Utc;

pub async fn example_json() -> impl IntoResponse {
    let mut data = HashMap::new();
    data.insert("framework".to_string(), "Axum".to_string());
    data.insert("templating".to_string(), "Askama".to_string());

    let response = ExampleJsonResponse {
        message: "This is a JSON API response".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        data,
    };

    Json(response)
}
```

### Route Definition

```rust
let public_router = Router::new()
    .route("/examples/json", get(example_json));
```

### JavaScript Usage

```javascript
fetch('/examples/json')
    .then(response => response.json())
    .then(data => console.log(data));
```

### Key Points

- **Use Json()**: Axum's Json wrapper handles serialization
- **Serialize types**: All response fields must implement Serialize
- **Content-Type**: Axum automatically sets `application/json`
- **Error handling**: Return appropriate HTTP status codes for errors

## Live Examples

The project includes live examples demonstrating these patterns:

### Contact Form Example

**URL:** `/examples/contact`

Demonstrates:
- Full-page Askama template
- HTMX form submission
- Validation error handling
- Success/error partial responses

**Try it:**
1. Navigate to `/examples/contact`
2. Submit the form with empty fields (see errors)
3. Submit with valid data (see success message)

### Counter Example

**URL:** `/examples/counter`

Demonstrates:
- Simple state updates with HTMX
- Partial template responses
- hx-trigger for initial load
- hx-include for form data

**Try it:**
1. Navigate to `/examples/counter`
2. Click increment/decrement buttons
3. Watch the counter update without page reload

### JSON API Example

**URL:** `/examples/json`

Demonstrates:
- JSON API endpoint
- Structured response
- Serialization with serde

**Try it:**
```bash
curl http://localhost:8888/examples/json
```

## Best Practices

### Template Organization

```
templates/
├── base.html              # Base layout (if using inheritance)
├── index.html             # Home page (full)
├── examples/
│   ├── contact.html       # Contact page (full)
│   ├── contact_success.html  # Success partial
│   ├── contact_errors.html   # Error partial
│   ├── counter.html       # Counter page (full)
│   └── counter_partial.html  # Counter partial
```

### Handler Organization

```rust
// src/controllers/examples.rs
pub mod examples {
    // Contact form handlers
    pub async fn contact_page() -> impl IntoResponse;
    pub async fn contact_submit() -> Response;

    // Counter handlers
    pub async fn counter_page() -> impl IntoResponse;
    pub async fn counter_increment() -> impl IntoResponse;
    pub async fn counter_decrement() -> impl IntoResponse;

    // JSON handlers
    pub async fn example_json() -> impl IntoResponse;
}
```

### Security Considerations

1. **CSRF Protection**: Always include CSRF tokens for state-changing requests
2. **Input Validation**: Validate all user input on the server
3. **Output Escaping**: Askama auto-escapes HTML by default
4. **Rate Limiting**: Apply rate limiting to form submissions
5. **Authentication**: Protect sensitive routes with middleware

### Performance Tips

1. **Template Caching**: Askama compiles templates at build time (no runtime cost)
2. **Connection Pooling**: Reuse database connections
3. **HTTP Caching**: Use cache headers for static content
4. **Compression**: Enable GZIP compression (already configured)
5. **Lazy Loading**: Use hx-trigger for loading content on demand

### Testing HTMX Endpoints

```rust
#[tokio::test]
async fn test_contact_submit_valid() {
    let form = ContactForm {
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        message: "This is a test message".to_string(),
    };

    let response = contact_submit(Form(form)).await;

    // Assert response is HTML
    // Assert success message is present
}
```

## Additional Resources

- [HTMX Documentation](https://htmx.org/docs/)
- [Askama Documentation](https://github.com/djc/askama)
- [Axum Documentation](https://docs.rs/axum/)
- [HTMX Examples](https://htmx.org/examples/)

## Summary

The HTMX + Askama pattern provides a simple, powerful approach to web development:

- **Full-page templates** for initial loads and navigation
- **Partial templates** for HTMX updates
- **Form validation** with error handling
- **Redirect vs partial** convention based on request type
- **JSON endpoints** for API consumers

This stack gives you interactivity without the complexity of modern JavaScript frameworks, while maintaining type safety and performance with Rust.

# CSRF Protection

This document describes the CSRF (Cross-Site Request Forgery) protection implementation in axum-kickoff.

## Overview

CSRF protection is enabled by default for all unsafe HTTP methods (POST, PUT, PATCH, DELETE). The implementation uses per-session CSRF tokens that are automatically generated and validated.

## How It Works

1. **Token Generation**: When a user first visits the site, a CSRF token is automatically generated and stored in their session.

2. **Token Validation**: For unsafe HTTP methods, the middleware validates that the request includes a valid CSRF token matching the one in the session.

3. **Token Submission**: CSRF tokens can be submitted via:
   - HTTP header: `X-CSRF-Token`
   - Form field: `csrf_token`

## Usage

### In Askama Templates

The CSRF token is automatically available in templates through the `csrf_token` variable:

```html
<form method="POST" action="/submit">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
    <!-- other form fields -->
    <button type="submit">Submit</button>
</form>
```

### With HTMX

For HTMX requests, you can configure HTMX to automatically include the CSRF token:

```html
<head>
    <meta name="htmx-config" content='{"csrfToken": "{{ csrf_token }}"}'>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
</head>
```

HTMX will then automatically include the CSRF token in requests.

### Manual Header Submission

For API clients or custom JavaScript, include the token in the `X-CSRF-Token` header:

```javascript
fetch('/api/v1/tokens', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'X-CSRF-Token': csrfToken
    },
    body: JSON.stringify(data)
});
```

### In Handlers

To get the CSRF token in a handler:

```rust
use axum::extract::Extension;
use crate::middleware::{SessionExtension, get_or_create_csrf_token};

async fn my_handler(
    Extension(session): Extension<SessionExtension>,
) -> Html<String> {
    let csrf_token = get_or_create_csrf_token(&session);
    // Use the token in your response
}
```

## Middleware

Two middleware functions are provided:

### `ensure_token`

Automatically ensures a CSRF token exists in the session. This is applied globally in the middleware stack, so every session will have a CSRF token available.

### `protect`

Validates CSRF tokens for unsafe HTTP methods. This is applied to specific routes that process form submissions or state-changing requests.

## Configuration

No additional configuration is required. CSRF protection works out of the box with the existing session configuration.

## Security Considerations

- CSRF tokens are 32-character alphanumeric strings generated using a cryptographically secure random number generator.
- Tokens are stored in signed session cookies, preventing tampering.
- Safe HTTP methods (GET, HEAD, OPTIONS) are exempt from CSRF validation.
- The middleware returns a 400 Bad Request error if CSRF validation fails.

## Testing

To test CSRF protection:

1. Make a POST request without a CSRF token - should return 400
2. Make a POST request with an invalid CSRF token - should return 400
3. Make a POST request with a valid CSRF token - should succeed
4. Make a GET request without a CSRF token - should succeed (safe method)

## HTMX Integration

The template includes HTMX configuration to automatically include the CSRF token in requests. This is done via the `htmx-config` meta tag in the HTML head:

```html
<meta name="htmx-config" content='{"csrfToken": "{{ csrf_token }}"}'>
```

This configuration tells HTMX to automatically include the CSRF token in the `X-CSRF-Token` header for all requests.

## Disabling CSRF Protection

To disable CSRF protection for specific routes (e.g., public API endpoints), simply don't apply the `protect` middleware to those routes. Note that this should only be done for endpoints that don't perform state-changing operations or that use alternative authentication mechanisms (e.g., API tokens).

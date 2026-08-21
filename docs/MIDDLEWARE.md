# Middleware

This document describes the middleware stack in axum-kickoff, including all middleware components, their order, and configuration.

## Overview

The middleware stack provides cross-cutting concerns for HTTP requests, including security, logging, authentication, and rate limiting. Middleware is applied in a specific order to ensure proper processing.

## Middleware Stack

Path normalization is applied as a top-level service around the router in `src/lib.rs::build_handler` before the remaining middleware stack. The rest of the stack is applied in `src/middleware/mod.rs` from outermost to innermost in this order:

1. **Debug Requests** - Full request logging (development only)
2. **Metrics** - Prometheus request metrics (`metrics` feature)
3. **Compression** - Gzip/Brotli response compression
4. **Request Body Timeout** - 30s timeout for reading the request body
5. **Timeout** - 30s timeout for the whole request
6. **Security Headers** - CSP (with nonces), HSTS, X-Frame-Options, etc.
7. **User Agent Validation** - Require a `User-Agent` header
8. **Panic Catcher** - Convert panics into 500 responses
9. **Request ID** - Generate and propagate `X-Request-ID`
10. **Error Handler** - Log 4xx/5xx responses
11. **Real IP Extraction** - Resolve client IP from proxy headers
12. **Request Logging** - Structured access log with request ID, real IP, and hashed `Authorization`/`Cookie` headers
13. **Session Management** - Cookie-based session middleware
14. **CSRF Token Ensure** - Create a CSRF token for the session
15. **Traffic Blocking** - Block IPs, matched routes, or header patterns
16. **Authentication** - Resolve session/API-token user and scopes
17. **Rate Limiting** - DB-backed token-bucket throttling
18. **CSRF Origin Verification** - Check origin on state-changing requests
19. **CORS** - Cross-origin resource sharing

## Individual Middleware

### Path Normalization

**Purpose:** Normalize request paths before routing.

**Implementation:** `src/middleware/normalize_path.rs`

**Configuration:** Applied as a top-level service around the router in `src/lib.rs::build_handler`.

**Behavior:**
- Collapses multiple consecutive slashes (`//` → `/`).
- Removes `.` segments.
- Resolves `..` segments and rejects paths that escape the root with `400 Bad Request`.
- Trims trailing slashes (except for the root path `/`).

**Example:**
```
/api//posts/  →  /api/posts
/foo/../bar  →  /bar
```

### Real IP Extraction

**Purpose:** Extract the real client IP from proxy headers (X-Forwarded-For, X-Real-IP).

**Implementation:** `src/middleware/real_ip.rs`

**Configuration:** Trust proxy headers by default.

**Headers Used:**
- `X-Forwarded-For`
- `X-Real-IP`

**Usage:**
```rust
use crate::middleware::real_ip::RealIp;

let ip = RealIp::from_request(req, &state).await?;
```

### Request Logging

**Purpose:** Log all HTTP requests with structured logging.

**Implementation:** `src/middleware/mod.rs::log_request`

**Configuration:** Uses tracing for structured logging.

**Log Fields:**
- `http.method`
- `http.url`
- `http.status_code`
- `duration_ms`
- `http.request.id`
- `network.client.ip`
- `http.user_agent`
- `http.request.headers.hashed_authorization`
- `http.request.headers.hashed_cookie`

Sensitive headers are SHA-256 hashed before logging. The raw `Authorization` and `Cookie` values never appear in logs.

**Example Log:**
```
INFO http: GET /api/posts -> 200 (1.234ms) http.method=GET http.url=/api/posts http.status_code=200 duration_ms=1 http.request.id=... network.client.ip=127.0.0.1 http.user_agent=... http.request.headers.hashed_authorization=... http.request.headers.hashed_cookie=...
```

### Error Handler

**Purpose:** Centralized error handling with consistent error responses.

**Implementation:** `src/middleware/error_handler.rs`

**Configuration:** Converts errors to appropriate HTTP responses.

**Error Types:**
- `AppError` - Application-specific errors
- `anyhow::Error` - Generic errors
- Panics - Converted to 500 responses

**Response Format:**
```json
{
  "error": "Error message",
  "status": 400
}
```

### Session Management

**Purpose:** Cookie-based session management for web authentication.

**Implementation:** `src/middleware/session.rs`

**Configuration:** Requires `SESSION_KEY` environment variable.

**Features:**
- Signed cookies with HMAC
- Session extraction and validation
- User context attachment

**Usage:**
```rust
use crate::middleware::session::SessionExtension;

let session = req.extensions().get::<SessionExtension>();
```

See [Authentication Documentation](AUTHENTICATION.md#session-management) for details.

### Panic Catcher

**Purpose:** Catch panics and convert to 500 responses.

**Implementation:** `tower_http::catch_panic::CatchPanicLayer`

**Configuration:** Applied automatically.

**Behavior:**
- Catches panics in handlers
- Returns 500 Internal Server Error
- Logs panic details

### User Agent Validation

**Purpose:** Block requests without User-Agent header.

**Implementation:** `src/middleware/require_user_agent.rs`

**Configuration:** Applied automatically to all routes.

**Behavior:**
- Returns 400 Bad Request if User-Agent is missing
- Helps prevent automated attacks

**Configuration:**
```bash
# No configuration needed - enabled by default
```

### Security Headers

**Purpose:** Add security headers to all responses.

**Implementation:** `src/middleware/security_headers.rs`

**Configuration:** Configurable via environment variables.

**Headers Added:**

#### Content Security Policy (CSP)

```bash
SECURITY_CSP_MODE=strict  # strict, permissive, or custom:CSP_STRING
```

**Strict Mode:**
```
Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none';
```

#### HTTP Strict Transport Security (HSTS)

```bash
SECURITY_HSTS_ENABLED=true
SECURITY_HSTS_MAX_AGE=31536000
SECURITY_HSTS_INCLUDE_SUBDOMAINS=true
SECURITY_HSTS_PRELOAD=true
```

**Header:**
```
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
```

#### X-Frame-Options

```bash
SECURITY_FRAME_OPTIONS=deny  # deny, sameorigin, or allow-from:URL
```

**Header:**
```
X-Frame-Options: deny
```

#### X-Content-Type-Options

**Header:**
```
X-Content-Type-Options: nosniff
```

#### X-XSS-Protection

**Header:**
```
X-XSS-Protection: 0
```

> The `X-XSS-Protection` header is intentionally set to `0` to avoid inconsistent browser behavior. CSP provides the actual XSS mitigation.

#### Referrer Policy

```bash
SECURITY_REFERRER_POLICY=strict-origin-when-cross-origin
```

**Header:**
```
Referrer-Policy: strict-origin-when-cross-origin
```

#### Permissions Policy

```bash
SECURITY_PERMISSIONS_POLICY=restrictive  # restrictive, permissive, or custom:POLICY_STRING
```

**Restrictive Mode:**
```
Permissions-Policy: geolocation=(), microphone=(), camera=()
```

### Timeout

**Purpose:** Apply timeout to entire request.

**Implementation:** `tower_http::timeout::TimeoutLayer`

**Configuration:** 30 seconds default.

**Behavior:**
- Returns 408 Request Timeout if exceeded
- Prevents hanging requests

**Configuration:**
```bash
# Currently hardcoded to 30s
# Can be made configurable if needed
```

### Request Body Timeout

**Purpose:** Apply timeout to request body reading.

**Implementation:** `tower_http::timeout::RequestBodyTimeoutLayer`

**Configuration:** 30 seconds default.

**Behavior:**
- Returns 408 Request Timeout if body reading exceeds timeout
- Prevents slowloris attacks

### Compression

**Purpose:** Compress response bodies with gzip.

**Implementation:** `tower_http::compression::CompressionLayer`

**Configuration:** Fastest compression level.

**Behavior:**
- Compresses responses > 1KB
- Respects Accept-Encoding header
- Reduces bandwidth usage

**Configuration:**
```bash
# Currently uses fastest compression
# Can be made configurable if needed
```

### Metrics

**Purpose:** Collect Prometheus metrics for monitoring.

**Implementation:** `src/middleware/metrics.rs`

**Configuration:** Feature-gated (`metrics` feature).

**Metrics Collected:**
- Request count by method and path
- Request duration by method and path
- Response status by method and path

**Enable:**
```bash
cargo run --features metrics --bin axum-kickoff -- server
```

**Access:**
```
GET /metrics
GET /api/private/metrics
```

When `METRICS_TOKEN` is set, requests must include `Authorization: Bearer <METRICS_TOKEN>`.

### Debug Requests

**Purpose:** Log full request details in development.

**Implementation:** `src/middleware/mod.rs::debug_requests`

**Configuration:** Only enabled in development environment.

**Behavior:**
- Logs full request details
- Helps with debugging
- Disabled in production

## Custom Middleware

### Adding Custom Middleware

To add custom middleware:

1. Create middleware function in `src/middleware/`:

```rust
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

pub async fn custom_middleware(
    req: Request,
    next: Next,
) -> Response {
    // Pre-processing
    tracing::info!("Custom middleware");

    // Call next middleware
    let response = next.run(req).await;

    // Post-processing
    response
}
```

2. Add to middleware stack in `src/middleware/mod.rs`:

```rust
pub fn apply_axum_middleware(state: AppState, router: Router<()>) -> Router {
    router
        .layer(from_fn(custom_middleware))
        // ... other middleware
}
```

### Conditional Middleware

Apply middleware conditionally:

```rust
let router = if env == Env::Development {
    router.layer(from_fn(debug_middleware))
} else {
    router
};
```

### Route-Specific Middleware

Apply middleware to specific routes:

```rust
router
    .route("/api/admin/*", get(admin_handler))
    .layer(from_fn(admin_auth_middleware))
```

## Middleware Order

The order of middleware is important:

1. **Outer layers** run first on request, last on response
2. **Inner layers** run last on request, first on response

**Example:**
```
Request:  A → B → C → Handler
Response: Handler → C → B → A
```

**Current Order (outer to inner):**
1. Path Normalization
2. Debug Requests (development only)
3. Metrics (`metrics` feature)
4. Compression
5. Request Body Timeout
6. Timeout
7. Security Headers
8. User Agent Validation
9. Panic Catcher
10. Request ID
11. Error Handler
12. Real IP Extraction
13. Request Logging
14. Session Management
15. CSRF Token Ensure
16. Traffic Blocking
17. Authentication
18. Rate Limiting
19. CSRF Origin Verification
20. CORS

## Configuration

### Environment Variables

See [Configuration Documentation](CONFIGURATION.md#security-configuration) for security header configuration.

### Feature Flags

- `metrics`: Enable Prometheus metrics

```bash
cargo run --features metrics --bin axum-kickoff -- server
```

## Testing

### Testing Middleware

Test middleware with integration tests:

```rust
use axum_kickoff::tests::{TestApp, AnonymousUser};

#[tokio::test]
async fn test_security_headers() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/endpoint").await;

    // Check for security headers
    assert!(response.headers().contains_key("strict-transport-security"));
    assert!(response.headers().contains_key("x-frame-options"));
}
```

## Troubleshooting

### Middleware Not Applied

- Check middleware order in `apply_axum_middleware`
- Verify middleware is added to router
- Check for compilation errors

### Security Headers Missing

- Verify environment variables are set
- Check `SECURITY_CSP_MODE` configuration
- Ensure headers are not being overridden

### Session Not Working

- Verify `SESSION_KEY` is set
- Check session middleware is applied
- Verify cookie domain matches application domain

### Timeout Too Short

- Adjust timeout values in middleware stack
- Consider increasing for long-running operations
- Monitor request duration metrics

## Best Practices

### DO

- Keep middleware focused and single-purpose
- Document middleware behavior
- Test middleware in isolation
- Use structured logging
- Configure appropriate timeouts
- Enable security headers in production
- Monitor middleware performance

### DON'T

- Don't put business logic in middleware
- Don't rely on middleware order without documentation
- Don't skip error handling in middleware
- Don't ignore performance impact
- Don't disable security headers in production

## See Also

- [Configuration Documentation](CONFIGURATION.md)
- [Authentication Documentation](AUTHENTICATION.md)
- [Architecture Documentation](ARCHITECTURE.md#middleware-stack)
- [Security Headers](#security-headers)

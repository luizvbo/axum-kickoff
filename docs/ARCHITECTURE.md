# Architecture

This document describes the high-level architecture of {{project-name}}, its design decisions, and how components interact.

## Overview

{{project-name}} is a single-crate Rust web application built on Axum, following production-grade patterns from crates.io while maintaining simplicity for general web applications.

## Design Principles

1. **Single-Crate Architecture**: All code in one crate for simplicity, with internal module organization
2. **Zero-Setup Development**: SQLite and local filesystem for instant development experience
3. **Production-Ready Patterns**: Battle-tested patterns from crates.io adapted for general use
4. **Gradual Complexity**: Start simple, upgrade to distributed systems when needed
5. **Type Safety**: Leverage Rust's type system throughout the stack
6. **Cost-Conscious**: Self-hostable with minimal external dependencies

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Client                                │
│                    (Browser / API Client)                    │
└────────────────────────┬────────────────────────────────────┘
                         │ HTTP/HTTPS
                         ▼
┌─────────────────────────────────────────────────────────────┐
│               Infrastructure Middleware (all routes)         │
│  CORS · Timeout/Body-Timeout · Compression · Security Headers│
│  Request ID · User-Agent · Panic · Error Handler · Real IP   │
│  Logging · Traffic Blocking{% if metrics %} · Metrics{% endif %}                          │
└────────────────────────┬────────────────────────────────────┘
                         │
         ┌───────────────┴───────────────┐
         ▼                               ▼
┌──────────────────┐          ┌──────────────────────────────┐
│ Operational      │          │  Application Middleware       │
│ routes: /health  │          │  Session · Authenticate ·     │
│ /static{% if metrics %} /metrics{% endif %}   │          │  Origin Verification          │
│ /swagger-ui      │          │  + per-route `route_layer`s:  │
│ /api-docs        │          │  require_auth · require_login │
└──────────────────┘          │  csrf_protect · rate_limit    │
                              └──────────────┬───────────────┘
                                             │
                                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      Router Layer                             │
│                    (Axum Router)                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ Auth Routes   │  │ API Routes   │  │ Web Routes   │     │
│  │ (OAuth sign-in)│  │ (REST API)   │  │ (HTML/HTMX)  │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Controller Layer                           │
│  auth · token · post · examples · home (all in               │
│  src/controllers/, template structs beside handlers)         │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Application State                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ Database │ │ Storage  │ │  Config  │ │ Rate     │       │
│  │  Pool    │ │ (local)  │ │          │ │ Limiter  │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
└────────────────────────┬────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
┌──────────────────┐ ┌────────────┐ ┌──────────────────┐
│   Database       │ │  Storage   │ │ Background worker│
│ (SQLite{% if database == "postgresql" %}/Postgres{% endif %})    │ │ (local FS) │ │ (DB job queue,   │
│                  │ │            │ │  `cargo run -- worker`)│
└──────────────────┘ └────────────┘ └──────────────────┘
```

## Component Architecture

### Application State (`App` struct)

The `App` struct in `src/app.rs` holds all application-wide components:

- **Database Connection Pool**: Toasty ORM connection pool
- **Storage Backend**: Pluggable storage for file uploads
- **Configuration**: Server and application configuration
- **Session Key**: For signing session cookies
- **Rate Limiter**: Database-backed token bucket rate limiter
{% if metrics %}- **Metrics**: Prometheus metrics (`metrics` Cargo feature)
{% endif %}
### Middleware Stack

Middleware is **not** one flat stack — `src/middleware/mod.rs` applies it in
two tiers:

1. **Infrastructure** (`apply_infra_middleware`, every route): CORS →
   timeout/body-timeout → compression → security headers → request ID →
   user-agent check → panic catch → request-format negotiation → error
   handler → real-IP extraction → request logging → traffic blocking
   (innermost).
2. **Application** (`apply_app_middleware`, only HTML/API routes): session
   cookies → `authenticate` (session + Bearer token resolution) → origin
   verification.
3. **Per-route `route_layer`s** in `src/router.rs`: `require_auth`,
   `require_login`, `csrf_protect`, `rate_limit` — enforcement at point of
   use, mirroring crates.io.

Operational routes (`/health`, `/static`, `/swagger-ui`, `/api-docs`{% if metrics %},
`/metrics`{% endif %}) see only tier 1 — they never create sessions, touch the
user database, or consume rate-limit tokens.

See [Middleware Documentation](MIDDLEWARE.md) for details.

### Controller Layer

Controllers in `src/controllers/` handle HTTP requests:

- **Auth Controller** (`auth.rs`): provider-generic OAuth authorize/callback, login, logout
- **Token Controller** (`token.rs`): API token creation/listing/revocation
- **Post Controller** (`post.rs`): CRUD + publish — the reference resource
- **Examples Controller** (`examples.rs`): HTMX/Alpine patterns (contact form, counter, JSON)
- **Home Controller** (`mod.rs`): home page, login page

Controllers follow these patterns:
- Use `AppResult<T>` for consistent error handling
- Implement `AuthCheck` for authorization
- Return structured JSON responses
- Validate input before processing

### Database Layer

The database layer uses Toasty ORM:

- **Models**: Defined in `src/models/` with Toasty macros
- **Migrations**: SQL generated by `migrate migration generate`, applied explicitly by `migrate`
- **Connection Pooling**: Built-in connection pooling
- **Query Builder**: Type-safe query construction

Supported databases:
- **SQLite**: Default for development (zero-setup)
- **PostgreSQL**: Production upgrade path

### Storage Layer

The storage layer in `src/storage.rs` is a thin abstraction over the local
filesystem (`STORAGE_PATH`, default `./local_uploads`) — the only backend
that exists. S3 design notes live in [Storage Documentation](STORAGE.md) but
no S3 or in-memory backend is implemented.

### Authentication System

Authentication supports multiple methods:

- **OAuth sign-in**: OAuth 2.0 flow (with PKCE) for the providers compiled in at generation time
- **Session-Based**: Signed cookie sessions
- **API Tokens**: Scoped tokens with fine-grained permissions

See [Authentication Documentation](AUTHENTICATION.md) for details.

### Rate Limiting

Rate limiting uses a database-backed token bucket (`rate_limit_buckets`
table) — persistent and shared across instances of the same database. There
is no in-memory or Redis backend; a Redis migration path is sketched in
`docs/rate-limiting-redis-upgrade.md` (design only).

See [Rate Limiting Documentation](RATE_LIMITING.md) for details.

## Request Flow

### Web Request (HTML/HTMX)

```
1. Client sends HTTP request
2. Middleware stack processes request
   - Extract real IP
   - Log request
   - Apply security headers
   - Check rate limits
   - Validate session
3. Router routes to controller
4. Controller processes request
   - Authenticate user
   - Validate input
   - Query database
   - Render template
5. Response sent through middleware
   - Add security headers
   - Compress if applicable
6. Client receives HTML response
```

### API Request (REST)

```
1. Client sends HTTP request with API token
2. Middleware stack processes request
   - Extract real IP
   - Log request
   - Apply security headers
   - Check rate limits
   - Validate API token
   - Check token scopes
3. Router routes to controller
4. Controller processes request
   - Validate token scopes
   - Validate input
   - Query database
   - Return JSON response
5. Response sent through middleware
   - Add security headers
   - Compress if applicable
6. Client receives JSON response
```

### OAuth Flow

```
1. User clicks a "Sign in with <provider>" button
2. Redirect to the provider's authorize endpoint
3. User authorizes application
4. Provider redirects to /api/v1/auth/<provider>/callback
5. Controller exchanges code for access token
6. Fetch user profile from the provider
7. Create/update user in database
8. Create session cookie
9. Redirect to dashboard
```

## Data Flow

### Database Operations

```
Controller → Database Model → Toasty ORM → Database
           ← Query Result   ← Query Result ← Query Result
```

### File Uploads

```
Controller → Storage Backend → File System / S3
           ← File URL        ← Upload Result
```

### Authentication

```
Request → Session Middleware → Session Cookie → User Lookup
         → API Token Middleware → Bearer Token → Token Validation
```

## Configuration Management

Configuration is loaded from environment variables:

- **Server Configuration**: IP, port, domain name
- **Database Configuration**: Connection URL, pool settings
- **Session Configuration**: Session key, cookie settings
- **OAuth Configuration**: Client ID, client secret, redirect URI
- **Storage Configuration**: Backend type, credentials
- **Rate Limiting Configuration**: Rates, burst sizes

See [Configuration Documentation](CONFIGURATION.md) for details.

## Error Handling

Error handling follows a structured approach:

- **AppError Trait**: Custom error types with `thiserror`
- **AppResult<T>**: Type alias for `anyhow::Result<T>`
- **Error Builders**: Helper functions for common HTTP errors
- **Middleware**: Centralized error handling middleware
- **User-Friendly Messages**: Actionable errors without sensitive data

See [Error Handling in Development Documentation](DEVELOPMENT.md) for details.

## Testing Architecture

Testing infrastructure in `src/tests/`:

- **TestApp**: Creates test-ready application with isolated database
- **RequestHelper Trait**: Makes HTTP requests in tests
- **Response Wrapper**: Helper methods for assertions
- **Test Builders**: Builder pattern for test data
- **Snapshot Testing**: `insta` for API response validation

See [Testing Documentation](TESTING.md) for details.

## Observability

### Logging

- **Structured Logging**: JSON format with tracing
- **Log Levels**: Configurable via `RUST_LOG`
- **Spans**: Request-scoped context
- Logs go to **stdout** only (JSON in production); QuickWit ingestion is a
  how-to ([quickwit-integration.md](quickwit-integration.md)), not a
  built-in integration.
{% if metrics %}
### Metrics

- **Prometheus**: `/metrics` and `/api/private/metrics` endpoints
- **Request Metrics**: counts, response times via middleware instrumentation
- Optional `METRICS_TOKEN` bearer protection
{% else %}
### Metrics

Not compiled in — this project was generated without the `metrics` option.
{% endif %}

## Security Architecture

### Authentication

- **OAuth sign-in**: OAuth 2.0 with PKCE
- **Session Cookies**: Signed with HMAC
- **API Tokens**: Hashed with SHA-256
- **Token Scopes**: Fine-grained permissions

### Authorization

- **Endpoint Scopes**: Read, create, update, delete, admin
- **Resource Scopes**: Resource-level access control
- **AuthCheck Pattern**: Declarative authorization

### Security Headers

- **Content Security Policy**: XSS protection
- **HSTS**: HTTPS enforcement
- **X-Frame-Options**: Clickjacking protection
- **X-Content-Type-Options**: MIME sniffing protection

### Rate Limiting

- **Per-Action Limits**: Different limits for different actions
- **Token Bucket**: Fair throttling algorithm
- **Shared State**: database backend; shared across instances on the same DB

## Scalability Considerations

### Single-Instance Deployment

Default configuration supports single-instance deployments:

- **Database-Backed Rate Limiting**: works out of the box on SQLite
- **SQLite Database**: Zero-setup, sufficient for moderate load
- **Local Storage**: Simple file uploads

### Multi-Instance Deployment

For horizontal scaling:

- **PostgreSQL**: Replace SQLite for shared database (rate-limit buckets and
  the worker queue become shared automatically)
- **Shared `STORAGE_PATH`**: block/shared storage for uploads (no S3 backend)
- **Load Balancer**: Nginx, HAProxy, or cloud LB

See [Deployment Documentation](DEPLOYMENT.md) for scaling guidance.

## Technology Choices

### Why Axum?

- **Modern**: Built on Tokio, async-first
- **Type-Safe**: Leverages Rust's type system
- **Extensible**: Tower middleware ecosystem
- **Performance**: Excellent performance characteristics

### Why Toasty?

- **Ergonomic**: Simple, intuitive API
- **Type-Safe**: Compile-time query validation
- **Zero-Setup**: SQLite support for development
- **Migrations**: Built-in schema management

### Why HTMX + Alpine.js?

- **Simplicity**: No complex build pipeline
- **Progressive Enhancement**: Works without JavaScript
- **Small Bundle**: Minimal JavaScript overhead
- **Server-Side Rendering**: Better SEO and performance

### Why SQLite?

- **Zero-Setup**: No database server required
- **Fast**: Excellent performance for read-heavy workloads
- **Reliable**: ACID compliant, battle-tested
- **Portable**: Single file database

## Future Enhancements

See [Roadmap](ROADMAP.md) for planned features and improvements.

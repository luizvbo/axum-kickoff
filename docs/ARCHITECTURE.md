# Architecture

This document describes the high-level architecture of axum-kickoff, its design decisions, and how components interact.

## Overview

axum-kickoff is a single-crate Rust web application built on Axum, following production-grade patterns from crates.io while maintaining simplicity for general web applications.

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
│                    Middleware Stack                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ Security │ │   CORS   │ │Rate Limit│ │  Logging │       │
│  │  Headers │ │          │ │          │ │          │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │  Session │ │ API Token│ │Real IP   │ │  Error   │       │
│  │  Mgmt    │ │  Auth    │ │Extraction│ │ Handler  │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      Router Layer                             │
│                    (Axum Router)                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ Auth Routes   │  │ API Routes   │  │ Web Routes   │     │
│  │ (GitHub OAuth)│  │ (REST API)   │  │ (HTML/HTMX)  │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Controller Layer                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ Auth Controller│ │Token Controller│ │User Controller│   │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Application State                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ Database │ │ Storage  │ │  Config  │ │ Rate     │       │
│  │  Pool    │ │ Backend  │ │          │ │ Limiter  │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
└────────────────────────┬────────────────────────────────────┘
                         │
         ┌───────────────┴───────────────┐
         ▼                               ▼
┌──────────────────┐          ┌──────────────────┐
│   Database       │          │    Storage      │
│  (SQLite/PG)     │          │ (Local/S3)      │
└──────────────────┘          └──────────────────┘
```

## Component Architecture

### Application State (`App` struct)

The `App` struct in `src/app.rs` holds all application-wide components:

- **Database Connection Pool**: Toasty ORM connection pool
- **Storage Backend**: Pluggable storage for file uploads
- **Configuration**: Server and application configuration
- **Session Key**: For signing session cookies
- **Rate Limiter**: Token bucket rate limiter
- **Metrics**: Prometheus metrics (feature-gated)

### Middleware Stack

Middleware is applied by `src/middleware/mod.rs` and wrapped with path normalization in `src/lib.rs::build_handler`. From outermost to innermost the layers are:

1. **Path Normalization**: Collapse slashes, resolve `..`, trim trailing `/`
2. **Debug Requests**: Full request logging (development only)
3. **Metrics**: Prometheus request metrics (`metrics` feature)
4. **Compression**: Gzip/Brotli response compression
5. **Request Body Timeout**: 30s timeout for reading the body
6. **Timeout**: 30s timeout for the whole request
7. **Security Headers**: CSP (with nonces), HSTS, X-Frame-Options, etc.
8. **User Agent Validation**: Require a `User-Agent` header
9. **Panic Catcher**: Convert panics into 500 responses
10. **Request ID**: Generate and propagate `X-Request-ID`
11. **Error Handler**: Log 4xx/5xx responses
12. **Real IP Extraction**: Resolve client IP from proxy headers
13. **Request Logging**: Structured access log with hashed sensitive headers
14. **Session Management**: Cookie-based session middleware
15. **CSRF Token Ensure**: Create a CSRF token for the session
16. **Traffic Blocking**: Block IPs, matched routes, or header patterns
17. **Authentication**: Resolve session/API-token user and scopes
18. **Rate Limiting**: DB-backed token-bucket throttling
19. **CSRF Origin Verification**: Check origin on state-changing requests
20. **CORS**: Cross-origin resource sharing

See [Middleware Documentation](MIDDLEWARE.md) for details.

### Controller Layer

Controllers in `src/controllers/` handle HTTP requests:

- **Auth Controller**: GitHub OAuth, login, logout
- **Token Controller**: API token management
- **User Controller**: User management (planned)

Controllers follow these patterns:
- Use `AppResult<T>` for consistent error handling
- Implement `AuthCheck` for authorization
- Return structured JSON responses
- Validate input before processing

### Database Layer

The database layer uses Toasty ORM:

- **Models**: Defined in `src/models/` with Toasty macros
- **Migrations**: Explicit `migrate migration apply` subcommand (not automatic on server start)
- **Connection Pooling**: Built-in connection pooling
- **Query Builder**: Type-safe query construction

Supported databases:
- **SQLite**: Default for development (zero-setup)
- **PostgreSQL**: Production upgrade path

### Storage Layer

The storage layer in `src/storage.rs` provides a pluggable abstraction:

- **Local Filesystem**: Implemented; default for development and testing
- **S3 Compatible**: Planned (AWS S3, MinIO, DigitalOcean Spaces)
- **In-Memory**: Planned for tests

See [Storage Documentation](STORAGE.md) for details.

### Authentication System

Authentication supports multiple methods:

- **GitHub OAuth**: OAuth 2.0 flow with GitHub
- **Session-Based**: Signed cookie sessions
- **API Tokens**: Scoped tokens with fine-grained permissions

See [Authentication Documentation](AUTHENTICATION.md) for details.

### Rate Limiting

Rate limiting uses a database-backed token bucket algorithm:

- **Database-Backed**: The default implementation persists bucket state in the configured database (SQLite or PostgreSQL) using the `rate_limit_buckets` table.
- **Redis**: Optional for high-throughput distributed systems (not implemented by default).

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
1. User clicks "Login with GitHub"
2. Redirect to GitHub authorize endpoint
3. User authorizes application
4. GitHub redirects to callback endpoint
5. Controller exchanges code for access token
6. Fetch user profile from GitHub
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
- **Integration**: QuickWit for log analytics

See [QuickWit Integration](quickwit-integration.md) for details.

### Metrics

- **Prometheus**: Metrics endpoint (feature-gated)
- **Instance Metrics**: Memory, CPU, connections
- **Service Metrics**: Request counts, response times, error rates
- **Custom Metrics**: Business metrics as needed

## Security Architecture

### Authentication

- **GitHub OAuth**: OAuth 2.0 with PKCE
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
- **Distributed Support**: Database/Redis backends

## Scalability Considerations

### Single-Instance Deployment

Default configuration supports single-instance deployments:

- **DB-Backed Rate Limiting**: Token bucket state lives in the database
- **SQLite Database**: Zero-setup, sufficient for moderate load
- **Local Storage**: Simple file uploads

### Multi-Instance Deployment

For horizontal scaling:

- **PostgreSQL**: Replace SQLite for shared database
- **Redis Rate Limiting**: Distributed rate limiting
- **S3 Storage**: Shared file storage
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

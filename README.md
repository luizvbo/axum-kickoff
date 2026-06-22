# axum-kickoff

A production-ready Rust web application starter template built on [Axum](https://github.com/tokio-rs/axum), following best practices from the [crates.io](https://github.com/rust-lang/crates.io) backend implementation.

## Features

- **Modern Stack**: Axum 0.8 with Tokio async runtime
- **Database**: Toasty ORM with SQLite (zero-setup) with PostgreSQL migration path
- **Authentication**: GitHub OAuth, session-based auth, and scoped API tokens
- **Frontend**: Server-side rendering with Askama, HTMX, and Alpine.js
- **Security**: Comprehensive middleware (security headers, rate limiting, etc.)
- **Observability**: Structured logging with tracing
- **Testing**: Integration test infrastructure with snapshot testing
- **Storage**: Local filesystem storage (pluggable architecture for future backends)
- **Cost-Conscious**: Designed for self-hosting with minimal external dependencies

## Quick Start

### Prerequisites

- Rust (see `rust-toolchain.toml` for pinned version)
- SQLite (for development)

### Installation

```bash
# Clone the repository
git clone https://github.com/luizvbo/axum-kickoff.git
cd axum-kickoff

# Copy environment variables
cp .env.sample .env

# Edit .env with your configuration
# Required: GH_CLIENT_ID, GH_CLIENT_SECRET, SESSION_KEY, WEB_ALLOWED_ORIGINS

# Run the server
cargo run --bin server
```

The server will start on `http://localhost:8888` by default.

### Configuration

Set the following environment variables in `.env`:

```bash
# Server
PORT=8888
DOMAIN_NAME=localhost

# Database
DATABASE_URL=sqlite:axum-kickoff.db

# Session
SESSION_KEY=your-secret-key-min-64-bytes

# GitHub OAuth
GH_CLIENT_ID=your-github-client-id
GH_CLIENT_SECRET=your-github-client-secret
GH_REDIRECT_URI=http://localhost:8888/api/v1/auth/github/callback

# CORS
WEB_ALLOWED_ORIGINS=http://localhost:8888,http://127.0.0.1:8888

# Storage
STORAGE_PATH=./local_uploads
```

See [Configuration Documentation](docs/CONFIGURATION.md) for all available options.

## Documentation

- **[Getting Started Guide](docs/GETTING_STARTED.md)** - Detailed setup and first steps
- **[Database Guide](docs/DATABASE.md)** - Toasty ORM usage, migrations, and querying
- **[HTMX + Askama Patterns](docs/HTMX_ASKAMA_PATTERNS.md)** - Frontend patterns with live examples
- **[How-to Guides](docs/HOW_TO_GUIDES.md)** - Common tasks and patterns
- **[Architecture](docs/ARCHITECTURE.md)** - System architecture and design decisions
- **[Authentication](docs/AUTHENTICATION.md)** - Authentication system overview
- **[Configuration](docs/CONFIGURATION.md)** - Complete configuration reference
- **[Deployment](docs/DEPLOYMENT.md)** - Deployment guide for production
- **[Production Checklist](docs/PRODUCTION_CHECKLIST.md)** - Production deployment checklist
- **[Development](docs/DEVELOPMENT.md)** - Development workflow and contributing
- **[Testing](docs/TESTING.md)** - Testing guide and conventions
- **[Storage](docs/STORAGE.md)** - Storage abstraction guide
- **[Middleware](docs/MIDDLEWARE.md)** - Middleware documentation
- **[API Token Scopes](docs/api-token-scopes.md)** - API token permission system
- **[Roadmap](docs/ROADMAP.md)** - Future development plans

## Project Structure

```
axum-kickoff/
├── src/
│   ├── bin/           # Binary entry points
│   ├── controllers/   # HTTP request handlers
│   ├── middleware/    # Axum middleware
│   ├── models/        # Database models (Toasty)
│   ├── config/        # Configuration management
│   ├── util/          # Utility functions
│   ├── tests/         # Integration test infrastructure
│   └── ...
├── templates/         # Askama templates
├── static/           # Static assets
├── docs/             # Documentation
└── Cargo.toml        # Dependencies
```

## Key Components

### Authentication System

- **GitHub OAuth**: Seamless integration with GitHub authentication
- **Session Management**: Secure cookie-based sessions with signed cookies
- **API Tokens**: Scoped API tokens with fine-grained permissions (read, create, update, delete, admin)
- **Token Scopes**: Resource-level and endpoint-level access control

See [Authentication Documentation](docs/AUTHENTICATION.md) for details.

### Rate Limiting

- **In-Memory**: Token bucket algorithm for single-instance deployments
- **Database-Backed**: Optional SQLite/PostgreSQL backend for distributed systems
- **Redis Upgrade Path**: Optional Redis backend for high-throughput scenarios
- **Per-Action Limits**: Different limits for API requests, login attempts, file uploads, etc.

See [Rate Limiting Documentation](docs/RATE_LIMITING.md) for details.

### Storage Abstraction

- **Local Filesystem**: Default for development
- **S3 Compatible**: AWS S3, MinIO, DigitalOcean Spaces, etc.
- **In-Memory**: For testing
- **Pluggable**: Easy to add custom backends

See [Storage Documentation](docs/STORAGE.md) for details.

### Middleware Stack

| Component                      | Description                                                          | Status                                                                                |
| ------------------------------ | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| GitHub OAuth                   | Authenticate users via GitHub OAuth                                  | Implemented                                                                           |
| Session Management             | Secure cookie-based sessions for user authentication                 | Implemented                                                                           |
| Security Headers               | HTTP security headers (CSP, HSTS, X-Frame-Options, etc.)             | Implemented                                                                           |
| Request Logging                | Structured logging of all HTTP requests                              | Implemented                                                                           |
| Error Handling                 | Centralized error handling and response formatting                   | Implemented                                                                           |
| Real IP Extraction             | Extract client IP from headers (X-Forwarded-For, etc.)               | Implemented                                                                           |
| User Agent Validation          | Validate User-Agent header to block malicious bots                   | Implemented                                                                           |
| API Token Creation/List/Revoke | Manage scoped API tokens for programmatic access                     | Implemented                                                                           |
| API Token Auth Middleware      | Authenticate requests via Bearer tokens with scope validation        | Implemented (with CurrentUser/CurrentAuth extractors)                                 |
| Rate Limiting                  | Limit request rate to prevent abuse and DoS attacks                  | Core implemented / not applied globally                                               |
| Traffic Blocking               | Block requests from malicious IPs or patterns                        | Infrastructure exists / not wired globally                                            |
| CSRF Protection                | Prevent Cross-Site Request Forgery attacks on forms                  | Implemented (split middleware: csrf_protect, require_session_user, require_api_token) |
| CORS                           | Cross-Origin Resource Sharing for frontend-backend communication     | Implemented                                                                           |
| Metrics Endpoint               | Prometheus metrics for monitoring and observability                  | Feature-gated / partial                                                               |
| S3 Storage                     | Object storage backend for file uploads (AWS S3, MinIO, etc.)        | Planned                                                                               |
| Redis Rate Limiting            | Distributed rate limiting using Redis for multi-instance deployments | Planned                                                                               |
| Database-backed Rate Limiting  | Persistent rate limiting using SQLite/PostgreSQL                     | Planned                                                                               |
| QuickWit Integration           | Self-hosted error tracking and log aggregation                       | Planned                                                                               |
| OpenAPI                        | Auto-generated API documentation (Swagger/OpenAPI)                   | Planned                                                                               |
| Background Worker              | Async job processing for tasks like email, webhooks                  | Planned                                                                               |
| Email System                   | Transactional email sending (SMTP, SendGrid, etc.)                   | Planned                                                                               |
| Webhooks                       | Webhook delivery for event notifications                             | Planned                                                                               |
| Read Replicas                  | Database read replicas for scaling read-heavy workloads              | Planned                                                                               |

See [Middleware Documentation](docs/MIDDLEWARE.md) for details.

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test '*'

# Accept snapshot changes
cargo insta accept
```

### Code Generation

```bash
# Generate database models from Toasty schema
cargo run --bin toasty
```

### Feature Flags

- `metrics`: Enable Prometheus metrics endpoint

```bash
# Run with metrics
cargo run --bin server --features metrics
```

## Deployment

### Docker

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/server /usr/local/bin/
EXPOSE 3000
CMD ["server"]
```

### Environment Variables

See [Deployment Documentation](docs/DEPLOYMENT.md) for production deployment guides including:

- Docker deployment
- Systemd service configuration
- Nginx reverse proxy setup
- PostgreSQL migration
- Production security considerations

## Philosophy

axum-kickoff is designed with these principles:

1. **Simplicity First**: Single-crate architecture with clear module organization
2. **Zero-Setup Development**: SQLite and local filesystem for instant start
3. **Production-Ready Patterns**: Based on crates.io's battle-tested implementation
4. **Cost-Conscious**: Self-hostable with minimal external dependencies
5. **Gradual Complexity**: Start simple, upgrade features as needed
6. **Type Safety**: Leverage Rust's type system throughout

## Comparison with crates.io

This project adapts crates.io's production-grade patterns while simplifying for general web applications:

- **Single-crate application** vs 25+ crate workspace
- **Toasty/SQLite** vs Diesel/PostgreSQL (with migration path)
- **HTMX/Alpine.js** vs SvelteKit SPA
- **In-memory rate limiting** vs database-backed (with upgrade path)
- **QuickWit** vs Sentry for error tracking (self-hosted alternative)

See [Roadmap](docs/ROADMAP.md) for detailed comparison and implementation plans.

## Contributing

Contributions are welcome! Please see [Development Documentation](docs/DEVELOPMENT.md) for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Inspired by the [crates.io](https://github.com/rust-lang/crates.io) backend implementation
- Built with [Axum](https://github.com/tokio-rs/axum) and [Tokio](https://tokio.rs)
- Uses [Toasty](https://github.com/stepchowfun/toasty) for database ORM
- Frontend powered by [HTMX](https://htmx.org) and [Alpine.js](https://alpinejs.dev)

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
- [just](https://github.com/casey/just) (for running setup and other commands)
- Node.js and npm (for vendoring frontend dependencies)

### Installation

```bash
# Clone the repository
git clone https://github.com/luizvbo/axum-kickoff.git
cd axum-kickoff

# Install dependencies and vendor JS libraries (HTMX, Alpine.js)
just setup

# Copy environment variables
cp .env.sample .env

# Edit .env with your configuration
# Required: GH_CLIENT_ID, GH_CLIENT_SECRET, SESSION_KEY, WEB_ALLOWED_ORIGINS

# Apply the database schema, then run the server
cargo run --bin axum-kickoff -- migrate migration apply
cargo run --bin axum-kickoff -- server
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

- **[Documentation Index](docs/INDEX.md)** — Curated reading order and feature map
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
| Session Management             | Secure cookie-based sessions with signed, `HttpOnly` cookies         | Implemented                                                                           |
| Security Headers               | CSP (with nonces), HSTS, X-Frame-Options, etc.                       | Implemented                                                                           |
| Request Logging                | Structured logging with request ID, real IP, and hashed headers      | Implemented                                                                           |
| Error Handling                 | Centralized `AppError` / `AppResult` handling                        | Implemented                                                                           |
| Real IP Extraction             | Extract client IP from `X-Forwarded-For` / `Forwarded`               | Implemented                                                                           |
| User Agent Validation          | Require a `User-Agent` header                                        | Implemented                                                                           |
| API Token Management           | Create, list, and revoke scoped API tokens                           | Implemented                                                                           |
| API Token Auth                 | Bearer token authentication with scope checks                        | Implemented (`CurrentUserId` / `AuthCheck`)                                           |
| Rate Limiting                  | DB-backed token bucket, per-action limits                            | Implemented                                                                           |
| Traffic Blocking               | Block IPs, matched routes, and header patterns                       | Implemented                                                                           |
| CSRF Protection                | Per-session tokens and origin verification                           | Implemented                                                                           |
| CORS                           | Cross-Origin Resource Sharing for configured origins                 | Implemented                                                                           |
| Request ID Middleware          | Unique request IDs in tracing spans and `X-Request-ID`               | Implemented                                                                           |
| Metrics Endpoint               | Prometheus metrics, token-protected in production                    | Implemented (`metrics` feature)                                                       |
| Path Normalization             | Collapse slashes, resolve `..`, trim trailing `/`                    | Implemented                                                                           |
| OpenAPI / Swagger UI           | Auto-generated API docs at `/swagger-ui`                             | Implemented                                                                           |
| Background Worker              | Polling job runner with exponential backoff                          | Implemented (built-in `CleanupJob`)                                                   |
| Local Storage                  | Filesystem file uploads                                              | Implemented                                                                           |
| S3 Storage                     | Object storage backend                                               | Planned                                                                               |
| Redis Rate Limiting            | Distributed rate limiting using Redis                                | Planned                                                                               |
| Quickwit Integration           | Self-hosted log analytics                                            | Documented / optional                                                                 |
| Email System                   | Transactional email sending                                          | Planned                                                                               |
| Webhooks                       | Webhook delivery for event notifications                             | Planned                                                                               |
| Read Replicas                  | Database read replicas                                               | Planned                                                                               |

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

### Database Migrations

```bash
# Generate a migration after changing models
cargo run --bin axum-kickoff -- migrate migration generate

# Apply pending migrations
cargo run --bin axum-kickoff -- migrate migration apply
```

### Feature Flags

- `metrics`: Enable Prometheus metrics endpoint

```bash
# Run with metrics
cargo run --features metrics --bin axum-kickoff -- server
```

## Deployment

### Docker

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin axum-kickoff

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/axum-kickoff /usr/local/bin/
EXPOSE 8888
CMD ["axum-kickoff", "server"]
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
- **DB-backed rate limiting** vs Redis (with upgrade path)
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

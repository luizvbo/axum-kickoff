# {{project-name}}

A production-ready Rust web application built on [Axum](https://github.com/tokio-rs/axum), following best practices from the [crates.io](https://github.com/rust-lang/crates.io) backend implementation.

> Generated with [cargo-generate](https://github.com/cargo-generate/cargo-generate) from
> [axum-kickoff](https://github.com/luizvbo/axum-kickoff).

## Features

- **Modern Stack**: Axum 0.8 with Tokio async runtime
- **Database**: Toasty ORM with SQLite (zero-setup) or PostgreSQL
- **Authentication**: OAuth sign-in (GitHub, Google, Facebook — selectable at generation), session-based auth, and scoped API tokens
- **Frontend**: Server-side rendering with Askama, HTMX, and Alpine.js
- **Security**: Comprehensive middleware (security headers, rate limiting, CSRF, etc.)
- **Testing**: Integration test infrastructure with snapshot testing
- **Storage**: Local filesystem storage (pluggable architecture for future backends)
- **Background Jobs**: Built-in worker for async job processing
- **API Docs**: OpenAPI/Swagger UI out of the box (utoipa)
- **Optional subsystems** (selected at generation time): Prometheus metrics, Sentry error reporting, jemalloc allocator

## Quick Start

### Prerequisites

- Rust (see `rust-toolchain.toml` for pinned version)
- [just](https://github.com/casey/just) (for running setup and other commands)
- Node.js and npm (for vendoring frontend dependencies)

### Installation

```bash
# Install dependencies and vendor JS libraries (HTMX, Alpine.js)
just setup

# Copy environment variables
cp .env.sample .env

# Edit .env with your configuration
# Required: SESSION_KEY, WEB_ALLOWED_ORIGINS, and the client credentials for
# each OAuth provider you want to enable (e.g. GH_CLIENT_ID/GH_CLIENT_SECRET).
# For local development also keep APP_ENV=development (the default when unset
# is production, which enables Secure cookies, JSON logs, and disables /debug)

# Run the server
cargo run --bin {{project-name}} -- server
```

The server will start on `http://localhost:8888` by default.

### Configuration

Set the following environment variables in `.env`:

```bash
# Environment (defaults to production when unset)
APP_ENV=development

# Server
PORT=8888
DOMAIN_NAME=localhost

# Database
DATABASE_URL=sqlite:./{{crate_name}}.db

# Session
SESSION_KEY=your-secret-key-min-64-bytes

# OAuth providers — each is enabled when both its credentials are set.
# Which providers exist is chosen at generation time (oauth_* options).
{% if oauth_github %}# GitHub
GH_CLIENT_ID=your-github-client-id
GH_CLIENT_SECRET=your-github-client-secret
GH_REDIRECT_URI=http://localhost:8888/api/v1/auth/github/callback
{% endif %}{% if oauth_google %}# Google
GOOGLE_CLIENT_ID=your-google-client-id
GOOGLE_CLIENT_SECRET=your-google-client-secret
GOOGLE_REDIRECT_URI=http://localhost:8888/api/v1/auth/google/callback
{% endif %}{% if oauth_facebook %}# Facebook
FACEBOOK_CLIENT_ID=your-facebook-app-id
FACEBOOK_CLIENT_SECRET=your-facebook-app-secret
FACEBOOK_REDIRECT_URI=http://localhost:8888/api/v1/auth/facebook/callback
{% endif %}

# CORS
WEB_ALLOWED_ORIGINS=http://localhost:8888,http://127.0.0.1:8888

# Storage
STORAGE_PATH=./local_uploads
```

See [Configuration Documentation](docs/CONFIGURATION.md) for all available options.

## Using this repository as a template

This repository is a [cargo-generate](https://github.com/cargo-generate/cargo-generate) template:

```bash
cargo install cargo-generate
cargo generate --git https://github.com/luizvbo/axum-kickoff --name my-app
```

You'll be prompted for the options below (or pass `-d <key>=<value>` to skip prompts):

| Option | Values | Default | Effect |
| ------ | ------ | ------- | ------ |
| `project-name` | any crate-safe name | asked once | Package name, binary name, session cookie (`<crate_name>_session`), default SQLite file (`<crate_name>.db`), PostgreSQL `application_name`, API title, user-agent strings, and the on-page application branding |
| `database` | `sqlite` / `postgresql` | `sqlite` | `sqlite`: zero-setup file database. `postgresql`: adds the Toasty Postgres driver and a `postgresql://` default `DATABASE_URL`. SQLite stays enabled either way — the test suite uses in-memory SQLite regardless |
| `oauth_github` | `true` / `false` | `true` | Compiles in the GitHub OAuth provider; enabled at runtime when `GH_CLIENT_ID`/`GH_CLIENT_SECRET` are set |
| `oauth_google` | `true` / `false` | `false` | Same for Google (`GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`) |
| `oauth_facebook` | `true` / `false` | `false` | Same for Facebook (`FACEBOOK_CLIENT_ID`/`FACEBOOK_CLIENT_SECRET`) |
| `metrics` | `true` / `false` | `false` | Adds the `prometheus` dependency, a `metrics` feature in `default`, the `/metrics` endpoint, and request instrumentation middleware |
| `sentry` | `true` / `false` | `false` | Adds `sentry` + `sentry-tracing` dependencies and feature, error-event capture via the tracing layer, and `SENTRY_DSN` support |
| `jemalloc` | `true` / `false` | `false` | Adds `tikv-jemallocator` and sets jemalloc as the global allocator |

The background worker, filesystem storage, and Swagger/OpenAPI subsystems are
unconditional parts of the application — they are not template options.

> **PostgreSQL note**: the checked-in migrations under `migrations/` are written
> in SQLite dialect (`AUTOINCREMENT`). When `database=postgresql` is selected
> the Postgres driver is compiled in and `DATABASE_URL` defaults to Postgres,
> but the migration SQL must be adapted for PostgreSQL before `migrate` will
> run against a real Postgres database. The test suite is unaffected — it runs
> on in-memory SQLite.

### Non-interactive generation

```bash
cargo generate --git https://github.com/luizvbo/axum-kickoff \
  --name my-app --silent \
  -d database=postgresql -d metrics=true -d sentry=true -d jemalloc=true
```

See the "Working on this template" section in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for how the template itself is
developed and verified.

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
{{project-name}}/
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

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Accept snapshot changes
cargo insta accept
```

### Database Migrations

```bash
# Apply pending migrations
cargo run --bin {{project-name}} -- migrate migration apply

# Generate a new migration after model changes
cargo run --bin {{project-name}} -- migrate migration generate
```

## Philosophy

{{project-name}} is designed with these principles:

1. **Simplicity First**: Single-crate architecture with clear module organization
2. **Zero-Setup Development**: SQLite and local filesystem for instant start
3. **Production-Ready Patterns**: Based on crates.io's battle-tested implementation
4. **Cost-Conscious**: Self-hostable with minimal external dependencies
5. **Gradual Complexity**: Start simple, upgrade features as needed
6. **Type Safety**: Leverage Rust's type system throughout

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

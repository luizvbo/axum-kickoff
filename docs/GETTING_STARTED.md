# Getting Started

This guide will help you get {{project-name}} up and running on your local machine.

## Prerequisites

- **Rust**: the version pinned in `rust-toolchain.toml` ([Install Rust](https://rustup.rs/))
- **Git**: For cloning the repository
- **SQLite**: Usually pre-installed on most systems, or install via package manager

### Installing Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Installing SQLite

**Ubuntu/Debian:**
```bash
sudo apt-get install sqlite3 libsqlite3-dev
```

**macOS:**
```bash
brew install sqlite
```

**Windows:**
Download from the [SQLite site](https://www.sqlite.org/download.html)

## Installation

### 1. Get the Project

This project was generated with `cargo generate`; you already have the
checkout. If you generated it with `--vcs none`, initialize git yourself:

```bash
cd {{project-name}}
git init
```

### 2. Set Up Environment Variables

Copy the sample environment file:

```bash
cp .env.sample .env
```

Edit `.env` with your configuration. The minimum required variables are:

```bash
# Application environment: defaults to `production` when unset, so set this
# explicitly for local development
APP_ENV=development

# Server Configuration
PORT=8888
DOMAIN_NAME=localhost

# Database
DATABASE_URL=sqlite:./{{crate_name}}.db

# Session Key (generate a secure random key)
SESSION_KEY=your-secret-key-minimum-64-bytes-long

{% if oauth_github %}# GitHub OAuth (enabled when both ID and secret are set)
GH_CLIENT_ID=your_github_client_id
GH_CLIENT_SECRET=your_github_client_secret
GH_REDIRECT_URI=http://localhost:8888/api/v1/auth/github/callback
{% endif %}{% if oauth_google %}# Google OAuth
GOOGLE_CLIENT_ID=your_google_client_id
GOOGLE_CLIENT_SECRET=your_google_client_secret
GOOGLE_REDIRECT_URI=http://localhost:8888/api/v1/auth/google/callback
{% endif %}{% if oauth_facebook %}# Facebook OAuth
FACEBOOK_CLIENT_ID=your_facebook_app_id
FACEBOOK_CLIENT_SECRET=your_facebook_app_secret
FACEBOOK_REDIRECT_URI=http://localhost:8888/api/v1/auth/facebook/callback
{% endif %}

# CORS (required)
WEB_ALLOWED_ORIGINS=http://localhost:8888,http://127.0.0.1:8888

# Storage
STORAGE_PATH=./local_uploads
```

### 3. Generate OAuth Credentials

{% if oauth_github or oauth_google or oauth_facebook %}Each provider compiled into this project is enabled when both its client ID
and secret are set. Configure the ones you want to offer on the login page:

{% else %}No OAuth providers were selected at generation time. Skip this section.

{% endif %}{% if oauth_github %}#### GitHub

1. Go to [GitHub Developer Settings](https://github.com/settings/developers)
2. Click "New OAuth App"
3. Fill in the form:
   - **Application name**: {{project-name}} (or your app name)
   - **Homepage URL**: `http://localhost:8888`
   - **Authorization callback URL**: `http://localhost:8888/api/v1/auth/github/callback`
4. Click "Register application"
5. Copy the **Client ID** and generate a **Client Secret**
6. Add them to your `.env` as `GH_CLIENT_ID`/`GH_CLIENT_SECRET`

{% endif %}{% if oauth_google %}#### Google

1. Go to the [Google Cloud Console credentials page](https://console.cloud.google.com/apis/credentials)
2. Create an OAuth client ID of type "Web application"
3. Add `http://localhost:8888/api/v1/auth/google/callback` to the "Authorized redirect URIs"
4. Copy the **Client ID** and **Client Secret** into `.env` as
   `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`

{% endif %}{% if oauth_facebook %}#### Facebook

1. Create an app at the [Meta for Developers portal](https://developers.facebook.com/apps)
   and add the "Facebook Login" product
2. Add `http://localhost:8888/api/v1/auth/facebook/callback` under
   "Valid OAuth Redirect URIs" in the Facebook Login settings
3. Copy the **App ID** and **App Secret** into `.env` as
   `FACEBOOK_CLIENT_ID`/`FACEBOOK_CLIENT_SECRET`

{% endif %}

### 4. Generate a Secure Session Key

Generate a cryptographically secure session key:

```bash
# Using OpenSSL (48 bytes → a 64-character key, the required minimum)
openssl rand -base64 48

# Or using Python
python3 -c "import secrets; print(secrets.token_urlsafe(48))"
```

Use the output as your `SESSION_KEY` in `.env`.

### 5. Run Database Migrations

{{project-name}} uses Toasty ORM. The schema is defined in `src/models/` and
the migration files ship in `migrations/`. `server` does **not** apply them —
run them yourself before starting the app:

```bash
cargo run --bin {{project-name}} -- migrate
# or: just migration-apply
```

For development, SQLite creates the database file automatically on first
connect; `migrate` then creates the tables.

### 6. Start the Server

```bash
cargo run --bin {{project-name}} -- server
```

You should see output like:

```
INFO Connecting to database...
INFO Database connected successfully
INFO Listening at http://127.0.0.1:8888
```

### 7. Access the Application

Open your browser and navigate to:

```
http://localhost:8888
```

## First Steps

### 1. Test Authentication

Click a "Sign in with …" button on `/login` to authenticate via OAuth. This will:

- Redirect you to the provider
- Ask for authorization
- Redirect back to your application
- Create a user account
- Start a session

### 2. Create an API Token

After logging in, you can create API tokens for programmatic access via
`POST /api/v1/tokens`. The endpoint accepts the session cookie (with the
`X-CSRF-Token` header) or an existing token with the `create` scope:

```bash
# Log in via OAuth in the browser first, then reuse the session cookie:
curl -X POST http://localhost:8888/api/v1/tokens \
  -H "Cookie: {{crate_name}}_session=<signed-cookie-value>" \
  -H "X-CSRF-Token: <session-csrf-token>" \
  -H "Content-Type: application/json" \
  -d '{"name": "My Token"}'
```

The plaintext token is returned once in the response's `data.token` field
(`ako...`) — store it, it can't be recovered.

### 3. Test the API

Use your API token to make authenticated requests:

```bash
curl http://localhost:8888/api/v1/posts \
  -H "Authorization: Bearer YOUR_API_TOKEN"
```

## Project Structure

Familiarize yourself with the project structure:

```
{{project-name}}/
├── src/
│   ├── bin/
│   │   └── main.rs            # Entry point (server, background-worker, migrate subcommands)
│   ├── controllers/
│   │   ├── auth.rs            # Authentication endpoints
│   │   ├── post.rs            # Post CRUD endpoints
│   │   ├── token.rs           # API token management
│   │   └── examples.rs        # HTMX/Askama example handlers
│   ├── middleware/
│   │   ├── mod.rs             # Middleware stack
│   │   ├── session.rs         # Session management
│   │   ├── auth.rs            # Authentication (session + API token)
│   │   ├── csrf.rs            # CSRF protection
│   │   └── ...                # Other middleware
│   ├── models/
│   │   ├── user.rs            # User model
│   │   ├── token.rs           # API token model
│   │   ├── post.rs            # Post model
│   │   ├── rate_limit_bucket.rs  # Rate limiter state
│   │   └── background_job.rs  # Worker job queue
│   ├── config/
│   │   ├── mod.rs             # Configuration module
│   │   ├── base.rs            # Base configuration
│   │   ├── database.rs        # Database configuration
│   │   └── server.rs          # Server configuration
│   ├── util/
│   │   ├── auth.rs            # Authentication utilities
│   │   ├── errors.rs          # Error handling
│   │   └── ...                # Other utilities
│   ├── tests/
│   │   ├── test_app.rs        # Test application builder
│   │   ├── request_helper.rs  # HTTP request helpers
│   │   └── ...                # Test utilities
│   ├── worker/                # Background job worker
│   ├── app.rs                 # Application state
│   ├── db.rs                  # Database connection
│   ├── oauth.rs               # OAuth provider registry
│   ├── rate_limiter.rs        # Rate limiting
│   ├── storage.rs             # Storage abstraction
│   └── lib.rs                 # Library entry point
├── templates/                 # Askama templates
├── static/                    # Static assets (CSS, JS)
├── docs/                      # Documentation
├── .env.sample               # Sample environment variables
├── Cargo.toml                 # Rust dependencies
└── README.md                  # This file
```

## Common Tasks

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Accept snapshot changes
cargo insta accept
```

### Building for Production

```bash
# Build release binary
cargo build --release

# The binary will be at target/release/{{project-name}}
```

{% if metrics %}### Metrics

This project was generated with the `metrics` option, so the Prometheus
endpoint is already compiled in — no feature flag needed:

```bash
cargo run --bin {{project-name}} -- server
```

Metrics are available at `/metrics` (and `/api/private/metrics` when
`METRICS_TOKEN` is set).
{% else %}### Metrics

This project was generated without the `metrics` option — there is no
`/metrics` endpoint or metrics feature. Regenerate with `metrics=true` if
you need it.
{% endif %}

### Database Operations

```bash
# Run database migrations (apply pending migrations)
cargo run --bin {{project-name}} -- migrate

# Generate a new migration after changing models
cargo run --bin {{project-name}} -- migrate migration generate

# View SQLite database
sqlite3 {{crate_name}}.db

# Backup SQLite database
cp {{crate_name}}.db {{crate_name}}.db.backup
```

## Troubleshooting

### Port Already in Use

If port 8888 is already in use, change the port in `.env`:

```bash
PORT=3001
```

### Database Connection Errors

Ensure the `DATABASE_URL` is correct:

```bash
# For SQLite (file-based)
DATABASE_URL=sqlite:./{{crate_name}}.db

# Tests don't read DATABASE_URL — they use temporary SQLite files.
# Point TEST_DATABASE_URL elsewhere to run them on another backend.
```

### OAuth Errors

Common issues:

- **Redirect URI mismatch**: Ensure the provider's `*_REDIRECT_URI` matches exactly what you configured in its app settings
- **Client ID/Secret incorrect**: Double-check the provider's app settings. Setting only one of the pair is a startup error
- **HTTP vs HTTPS**: Providers require HTTPS for production OAuth callbacks
- **"Unknown or unconfigured OAuth provider"**: the provider wasn't selected at generation time, or its credentials aren't set

### Session Key Errors

Ensure your `SESSION_KEY` is at least 64 bytes long. Generate a new one:

```bash
openssl rand -base64 64
```

### Storage Permissions

Ensure the storage directory is writable:

```bash
mkdir -p ./local_uploads
chmod 755 ./local_uploads
```

## Next Steps

- Read the [Architecture Documentation](ARCHITECTURE.md) to understand the system design
- Check the [Configuration Documentation](CONFIGURATION.md) for all available options
- Review the [Authentication Documentation](AUTHENTICATION.md) to understand auth flows
- Explore the [Development Documentation](DEVELOPMENT.md) for contribution guidelines

## Getting Help

- Check the [Documentation](README.md#documentation) for detailed guides
- Review issues on the [axum-kickoff template repo](https://github.com/luizvbo/axum-kickoff/issues)
- Open a new issue if you encounter problems

## Additional Resources

- [Axum Documentation](https://docs.rs/axum/)
- [Toasty Documentation](https://github.com/stepchowfun/toasty)
- [HTMX Documentation](https://htmx.org/docs/)
- [Alpine.js Documentation](https://alpinejs.dev/)

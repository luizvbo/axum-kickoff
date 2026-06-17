# Getting Started

This guide will help you get axum-kickoff up and running on your local machine.

## Prerequisites

- **Rust**: 1.70 or later ([Install Rust](https://rustup.rs/))
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
Download from [SQLite官网](https://www.sqlite.org/download.html)

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/luizvbo/axum-kickoff.git
cd axum-kickoff
```

### 2. Set Up Environment Variables

Copy the sample environment file:

```bash
cp .env.sample .env
```

Edit `.env` with your configuration. The minimum required variables are:

```bash
# Server Configuration
PORT=8888
DOMAIN_NAME=localhost

# Database
DATABASE_URL=sqlite:axum-kickoff.db

# Session Key (generate a secure random key)
SESSION_KEY=your-secret-key-minimum-32-bytes-long

# GitHub OAuth (required for authentication)
GH_CLIENT_ID=your_github_client_id
GH_CLIENT_SECRET=your_github_client_secret
GH_REDIRECT_URI=http://localhost:8888/api/v1/auth/github/callback

# CORS (required)
WEB_ALLOWED_ORIGINS=http://localhost:8888,http://127.0.0.1:8888

# Storage
STORAGE_PATH=./local_uploads
```

### 3. Generate GitHub OAuth Credentials

To enable GitHub OAuth authentication:

1. Go to [GitHub Developer Settings](https://github.com/settings/developers)
2. Click "New OAuth App"
3. Fill in the form:
   - **Application name**: axum-kickoff (or your app name)
   - **Homepage URL**: `http://localhost:8888`
   - **Authorization callback URL**: `http://localhost:8888/api/v1/auth/github/callback`
4. Click "Register application"
5. Copy the **Client ID** and generate a **Client Secret**
6. Add these to your `.env` file

### 4. Generate a Secure Session Key

Generate a cryptographically secure session key:

```bash
# Using OpenSSL
openssl rand -base64 32

# Or using Python
python3 -c "import secrets; print(secrets.token_urlsafe(32))"
```

Use the output as your `SESSION_KEY` in `.env`.

### 5. Run Database Migrations

axum-kickoff uses Toasty ORM for database management. The schema is defined in `src/models/` and migrations are handled automatically on first run.

For development, SQLite will create the database file automatically on first startup.

### 6. Start the Server

```bash
cargo run --bin server
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

Click "Login with GitHub" to authenticate via OAuth. This will:

- Redirect you to GitHub
- Ask for authorization
- Redirect back to your application
- Create a user account
- Start a session

### 2. Create an API Token

After logging in, you can create API tokens for programmatic access:

```bash
# Via the web UI (when implemented)
# Navigate to Settings → API Tokens → Create Token

# Or via API (when implemented)
curl -X POST http://localhost:3000/api/tokens \
  -H "Authorization: Bearer YOUR_SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "My Token", "scopes": ["read"]}'
```

### 3. Test the API

Use your API token to make authenticated requests:

```bash
curl http://localhost:3000/api/health \
  -H "Authorization: Bearer YOUR_API_TOKEN"
```

## Project Structure

Familiarize yourself with the project structure:

```
axum-kickoff/
├── src/
│   ├── bin/
│   │   └── server.rs          # Server entry point
│   ├── controllers/
│   │   ├── auth.rs            # Authentication endpoints
│   │   └── token.rs           # API token management
│   ├── middleware/
│   │   ├── mod.rs             # Middleware stack
│   │   ├── session.rs         # Session management
│   │   ├── api_token.rs       # API token authentication
│   │   └── ...                # Other middleware
│   ├── models/
│   │   ├── user.rs            # User model
│   │   ├── token.rs           # API token model
│   │   └── oauth_github.rs    # GitHub OAuth model
│   ├── config/
│   │   ├── mod.rs             # Configuration module
│   │   ├── base.rs            # Base configuration
│   │   └── database.rs        # Database configuration
│   ├── util/
│   │   ├── auth.rs            # Authentication utilities
│   │   ├── errors.rs          # Error handling
│   │   └── ...                # Other utilities
│   ├── tests/
│   │   ├── test_app.rs        # Test application builder
│   │   ├── request_helper.rs  # HTTP request helpers
│   │   └── ...                # Test utilities
│   ├── app.rs                 # Application state
│   ├── db.rs                  # Database connection
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

# The binary will be at target/release/server
```

### Enabling Metrics

Build with the metrics feature flag:

```bash
cargo run --bin server --features metrics
```

Metrics will be available at `/metrics`.

### Database Operations

```bash
# Generate models from Toasty schema
cargo run --bin toasty

# View SQLite database
sqlite3 axum-kickoff.db

# Backup SQLite database
cp axum-kickoff.db axum-kickoff.db.backup
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
DATABASE_URL=sqlite:axum-kickoff.db

# For SQLite (in-memory, for testing)
DATABASE_URL=sqlite::memory:
```

### GitHub OAuth Errors

Common issues:

- **Redirect URI mismatch**: Ensure `GITHUB_REDIRECT_URI` matches exactly what you configured in GitHub
- **Client ID/Secret incorrect**: Double-check your GitHub OAuth app settings
- **HTTP vs HTTPS**: GitHub requires HTTPS for production OAuth callbacks

### Session Key Errors

Ensure your `SESSION_KEY` is at least 32 bytes long. Generate a new one:

```bash
openssl rand -base64 32
```

### Storage Permissions

Ensure the storage directory is writable:

```bash
mkdir -p ./uploads
chmod 755 ./uploads
```

## Next Steps

- Read the [Architecture Documentation](ARCHITECTURE.md) to understand the system design
- Check the [Configuration Documentation](CONFIGURATION.md) for all available options
- Review the [Authentication Documentation](AUTHENTICATION.md) to understand auth flows
- Explore the [Development Documentation](DEVELOPMENT.md) for contribution guidelines

## Getting Help

- Check the [Documentation](README.md#documentation) for detailed guides
- Review existing [Issues](https://github.com/luizvbo/axum-kickoff/issues)
- Open a new issue if you encounter problems

## Additional Resources

- [Axum Documentation](https://docs.rs/axum/)
- [Toasty Documentation](https://github.com/stepchowfun/toasty)
- [HTMX Documentation](https://htmx.org/docs/)
- [Alpine.js Documentation](https://alpinejs.dev/)

# Configuration

This document provides a complete reference for configuring axum-kickoff.

## Overview

Configuration is managed through environment variables. The application reads these variables at startup using the `dotenvy` crate, which loads them from a `.env` file in the project root.

## Quick Start

1. Copy the sample environment file:
   ```bash
   cp .env.sample .env
   ```

2. Edit `.env` with your configuration values

3. Start the application:
   ```bash
   cargo run --bin server
   ```

## Required Configuration

### Database

```bash
DATABASE_URL=sqlite:./axum_kickoff.db
```

**Options:**
- SQLite file: `sqlite:./path/to/database.db`
- SQLite in-memory: `sqlite::memory:`
- PostgreSQL: `postgresql://user:password@host:port/database`

**Default:** `sqlite:./axum_kickoff.db`

### Session Key

```bash
SESSION_KEY=your-secret-key-minimum-64-bytes-long-here
```

The session key is used to sign and encrypt cookies. It must be at least 64 bytes long for security.

**Generate a secure key:**
```bash
# Using OpenSSL
openssl rand -base64 64

# Using Python
python3 -c "import secrets; print(secrets.token_urlsafe(64))"
```

### CORS Origins

```bash
WEB_ALLOWED_ORIGINS=http://localhost:3000,http://127.0.0.1:3000
```

Comma-separated list of allowed CORS origins for API requests.

## Server Configuration

### Port

```bash
PORT=8888
```

The port the server listens on.

**Default:** `8888`

### Domain Name

```bash
DOMAIN_NAME=localhost
```

The domain name of the application. Used for cookie domains and URL generation.

**Default:** `localhost`

### Server IP

```bash
SERVER_IP=127.0.0.1
```

The IP address to bind to.

**Default:** `127.0.0.1`

### Worker Threads

```bash
SERVER_THREADS=4
```

Number of worker threads for the Tokio runtime. Leave unset for default (number of CPU cores).

### Max Blocking Threads

```bash
MAX_BLOCKING_THREADS=512
```

Maximum number of blocking threads for blocking operations (e.g., file I/O). Leave unset for default.

## Environment Detection

### Heroku

```bash
HEROKU=1
```

Set to any value to indicate running on Heroku. This sets the environment to `Production`.

### Docker

```bash
DEV_DOCKER=1
```

Set to any value to indicate running in Docker.

## GitHub OAuth Configuration

### Client ID

```bash
GITHUB_CLIENT_ID=your_github_client_id
```

Your GitHub OAuth application client ID.

### Client Secret

```bash
GITHUB_CLIENT_SECRET=your_github_client_secret
```

Your GitHub OAuth application client secret.

### Redirect URI

```bash
GITHUB_REDIRECT_URI=http://localhost:3000/auth/github/callback
```

The OAuth callback URL. Must match exactly what you configured in your GitHub OAuth app settings.

## Storage Configuration

### Storage Backend

```bash
STORAGE_BACKEND=local
```

The storage backend to use.

**Options:**
- `local`: Local filesystem
- `s3`: AWS S3 or S3-compatible storage
- `memory`: In-memory storage (for testing)

### Local Storage Path

```bash
STORAGE_LOCAL_PATH=./uploads
```

Directory path for local filesystem storage. Used when `STORAGE_BACKEND=local`.

### S3 Configuration

```bash
STORAGE_S3_BUCKET=your-bucket-name
STORAGE_S3_REGION=us-east-1
STORAGE_S3_ACCESS_KEY=your-access-key
STORAGE_S3_SECRET_KEY=your-secret-key
STORAGE_S3_ENDPOINT=https://s3.amazonaws.com
```

S3 configuration for AWS S3 or S3-compatible services (MinIO, DigitalOcean Spaces, etc.).

## Rate Limiting Configuration

Rate limiting uses a token bucket algorithm. Configure limits per action type.

### API Request Rate Limiting

```bash
RATE_LIMITER_API_REQUEST_RATE_SECONDS=1
RATE_LIMITER_API_REQUEST_BURST=10
```

- `RATE_SECONDS`: Time between token refills (seconds)
- `BURST`: Maximum burst size

### Login Attempt Rate Limiting

```bash
RATE_LIMITER_LOGIN_ATTEMPT_RATE_SECONDS=5
RATE_LIMITER_LOGIN_ATTEMPT_BURST=5
```

### Password Reset Rate Limiting

```bash
RATE_LIMITER_PASSWORD_RESET_RATE_SECONDS=60
RATE_LIMITER_PASSWORD_RESET_BURST=3
```

### File Upload Rate Limiting

```bash
RATE_LIMITER_FILE_UPLOAD_RATE_SECONDS=10
RATE_LIMITER_FILE_UPLOAD_BURST=5
```

### Form Submission Rate Limiting

```bash
RATE_LIMITER_FORM_SUBMISSION_RATE_SECONDS=30
RATE_LIMITER_FORM_SUBMISSION_BURST=10
```

## Security Configuration

### Blocked IPs

```bash
BLOCKED_IPS=192.168.1.100,10.0.0.50
```

Comma-separated list of blocked IP addresses.

### Blocked Routes

```bash
BLOCKED_ROUTES=/api/admin,/api/internal
```

Comma-separated list of blocked route patterns.

### Blocked Traffic by Headers

```bash
BLOCKED_TRAFFIC=User-Agent=BLOCKED_UAS
BLOCKED_UAS=/curl\/[\d]+\.[\d]+\.[\d]+/,bad-bot
```

Block traffic based on header values. Values starting and ending with `/` are treated as regex patterns.

### HSTS Configuration

```bash
SECURITY_HSTS_ENABLED=false
SECURITY_HSTS_MAX_AGE=31536000
SECURITY_HSTS_INCLUDE_SUBDOMAINS=true
SECURITY_HSTS_PRELOAD=false
```

HTTP Strict Transport Security (HSTS) configuration. Only effective with HTTPS.

- `SECURITY_HSTS_ENABLED`: Enable HSTS
- `SECURITY_HSTS_MAX_AGE`: Max-age in seconds (default: 31536000 = 1 year)
- `SECURITY_HSTS_INCLUDE_SUBDOMAINS`: Include subdomains directive
- `SECURITY_HSTS_PRELOAD`: Preload directive for browser preload lists

### Content Security Policy

```bash
SECURITY_CSP_MODE=strict
```

Content Security Policy mode.

**Options:**
- `strict`: Strict CSP (recommended)
- `permissive`: Permissive CSP
- `custom:CSP_STRING`: Custom CSP string

### Frame Options

```bash
SECURITY_FRAME_OPTIONS=deny
```

X-Frame-Options header.

**Options:**
- `deny`: Deny all framing
- `sameorigin`: Allow same origin framing
- `allow-from:URL`: Allow framing from specific URL

### Frame Ancestors

```bash
SECURITY_FRAME_ANCESTORS=https://trusted-domain.com
```

Custom frame ancestors for CSP (overrides frame-ancestors directive).

### Referrer Policy

```bash
SECURITY_REFERRER_POLICY=strict-origin-when-cross-origin
```

Referrer-Policy header.

**Options:**
- `no-referrer`
- `no-referrer-when-downgrade`
- `unsafe-url`
- `strict-origin-when-cross-origin`
- `same-origin`
- `strict-origin`
- `origin-when-cross-origin`

### Permissions Policy

```bash
SECURITY_PERMISSIONS_POLICY=restrictive
```

Permissions-Policy header.

**Options:**
- `restrictive`: Restrictive policy
- `permissive`: Permissive policy
- `custom:POLICY_STRING`: Custom policy string

## Logging Configuration

### Log Level

```bash
RUST_LOG=info
```

Set the logging level using the `RUST_LOG` environment variable.

**Options:**
- `error`: Only errors
- `warn`: Warnings and errors
- `info`: Info, warnings, and errors (default)
- `debug`: Debug, info, warnings, and errors
- `trace`: All log levels

### Module-Specific Logging

```bash
RUST_LOG=axum_kickoff=debug,tower_http=info
```

Set different log levels for specific modules.

## Testing Configuration

### Test Database URL

```bash
TEST_DATABASE_URL=sqlite::memory:
```

Database URL for tests. Defaults to in-memory SQLite if not set.

## Feature Flags

### Metrics

```bash
cargo run --bin server --features metrics
```

Enable Prometheus metrics endpoint at `/metrics`.

## Configuration Files

### .env.sample

The `.env.sample` file contains all available configuration options with default values and comments. Copy this to `.env` and customize for your environment.

### Loading Order

Configuration is loaded in this order:

1. System environment variables
2. `.env` file in project root
3. Default values in code

Later sources override earlier sources.

## Production Configuration

### Required for Production

For production deployment, ensure you have:

1. **Secure Session Key**: Generate a cryptographically secure 64+ byte key
2. **HTTPS**: Use HTTPS in production (required for HSTS, secure cookies)
3. **PostgreSQL**: Use PostgreSQL instead of SQLite for production
4. **Environment Variables**: Set all required environment variables
5. **CORS Origins**: Configure appropriate CORS origins
6. **Rate Limiting**: Configure appropriate rate limits
7. **Security Headers**: Enable HSTS and configure security headers

### Production Example

```bash
# Database
DATABASE_URL=postgresql://user:password@db.example.com:5432/axum_kickoff

# Session
SESSION_KEY=<generate-secure-64-byte-key>

# Server
PORT=3000
DOMAIN_NAME=example.com
SERVER_IP=0.0.0.0

# GitHub OAuth
GITHUB_CLIENT_ID=your_production_client_id
GITHUB_CLIENT_SECRET=your_production_client_secret
GITHUB_REDIRECT_URI=https://example.com/auth/github/callback

# CORS
WEB_ALLOWED_ORIGINS=https://example.com

# Storage
STORAGE_BACKEND=s3
STORAGE_S3_BUCKET=production-bucket
STORAGE_S3_REGION=us-east-1
STORAGE_S3_ACCESS_KEY=your_access_key
STORAGE_S3_SECRET_KEY=your_secret_key

# Security
SECURITY_HSTS_ENABLED=true
SECURITY_HSTS_MAX_AGE=31536000
SECURITY_HSTS_INCLUDE_SUBDOMAINS=true
SECURITY_HSTS_PRELOAD=true
SECURITY_CSP_MODE=strict
SECURITY_FRAME_OPTIONS=deny
SECURITY_REFERRER_POLICY=strict-origin-when-cross-origin

# Rate Limiting
RATE_LIMITER_API_REQUEST_RATE_SECONDS=1
RATE_LIMITER_API_REQUEST_BURST=100
RATE_LIMITER_LOGIN_ATTEMPT_RATE_SECONDS=5
RATE_LIMITER_LOGIN_ATTEMPT_BURST=5

# Logging
RUST_LOG=info
```

## Development Configuration

### Development Example

```bash
# Database
DATABASE_URL=sqlite:./axum_kickoff.db

# Session
SESSION_KEY=dev-session-key-for-local-development-only

# Server
PORT=3000
DOMAIN_NAME=localhost
SERVER_IP=127.0.0.1

# GitHub OAuth (use dev app)
GITHUB_CLIENT_ID=your_dev_client_id
GITHUB_CLIENT_SECRET=your_dev_client_secret
GITHUB_REDIRECT_URI=http://localhost:3000/auth/github/callback

# CORS
WEB_ALLOWED_ORIGINS=http://localhost:3000,http://127.0.0.1:3000

# Storage
STORAGE_BACKEND=local
STORAGE_LOCAL_PATH=./uploads

# Security (relaxed for development)
SECURITY_HSTS_ENABLED=false
SECURITY_CSP_MODE=permissive

# Rate Limiting (relaxed for development)
RATE_LIMITER_API_REQUEST_RATE_SECONDS=1
RATE_LIMITER_API_REQUEST_BURST=1000

# Logging
RUST_LOG=debug
```

## Troubleshooting

### Configuration Not Loading

Ensure:
- `.env` file exists in project root
- `.env` file is readable
- Environment variables are properly formatted (no spaces around `=`)
- No typos in variable names

### Session Key Too Short

Error: "Session key must be at least 64 bytes"

Generate a longer key:
```bash
openssl rand -base64 64
```

### Database Connection Failed

Check:
- `DATABASE_URL` is correctly formatted
- Database server is running (for PostgreSQL)
- Database user has correct permissions
- SQLite file path is writable (for SQLite)

### CORS Errors

Ensure:
- `WEB_ALLOWED_ORIGINS` includes your frontend URL
- No trailing slashes in origins
- Comma-separated list has no spaces

## Security Best Practices

1. **Never commit `.env` to version control**
2. **Use strong, randomly generated session keys**
3. **Rotate session keys periodically**
4. **Use environment-specific configurations**
5. **Enable HSTS in production**
6. **Configure appropriate CORS origins**
7. **Use PostgreSQL in production**
8. **Set appropriate rate limits**
9. **Enable security headers**
10. **Monitor configuration changes**

## See Also

- [Getting Started Guide](GETTING_STARTED.md)
- [Deployment Guide](DEPLOYMENT.md)
- [Security Headers Documentation](MIDDLEWARE.md#security-headers)
- [Rate Limiting Documentation](RATE_LIMITING.md)

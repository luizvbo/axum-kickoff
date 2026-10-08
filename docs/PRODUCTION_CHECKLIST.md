# Production Deployment Checklist

Use this checklist when deploying {{project-name}} to production.

## Security

### Session Key
- [ ] Generate a cryptographically secure 64+ byte session key
  ```bash
  openssl rand -base64 64
  ```
- [ ] Set `SESSION_KEY` environment variable with the generated key
- [ ] Never commit the session key to version control
- [ ] Store the key securely (e.g., environment variable manager, secrets manager)

### HTTPS
- [ ] Enable HTTPS using a reverse proxy (Nginx, Caddy, Traefik)
- [ ] Obtain SSL/TLS certificate (Let's Encrypt recommended)
- [ ] Configure HTTP to HTTPS redirect
- [ ] Test HTTPS configuration with SSL Labs test

### Secure Cookies
- [ ] Ensure cookies are only sent over HTTPS (automatic with HTTPS)
- [ ] Set appropriate cookie flags in production
- [ ] Verify `SameSite` attribute is set correctly
- [ ] Test cookie behavior in different browsers

### Database Security
- [ ] Use PostgreSQL in production (not SQLite)
- [ ] Create a dedicated database user with minimal permissions
- [ ] Enable database SSL connections
- [ ] Use strong database password
- [ ] Configure database connection pooling
- [ ] Set up database backups

### OAuth Configuration
- [ ] Create production OAuth applications for each enabled provider
- [ ] Set production callback URLs (HTTPS) in each provider's app settings
- [ ] Use production client IDs and secrets
- [ ] Verify each redirect URI matches exactly
- [ ] Test the OAuth flow for each provider in production environment

### CORS
- [ ] Configure `WEB_ALLOWED_ORIGINS` with production domains
- [ ] Remove localhost from allowed origins
- [ ] Test CORS configuration with production URLs

### Rate Limiting
- [ ] Configure appropriate rate limits for production
- [ ] Set `RATE_LIMITER_API_REQUEST_RATE_SECONDS` and `BURST`
- [ ] Configure `RATE_LIMITER_LOGIN_ATTEMPT` limits
- [ ] Consider Redis for distributed rate limiting (if needed)
- [ ] Test rate limiting behavior

### Security Headers
- [ ] Enable HSTS: `SECURITY_HSTS_ENABLED=true`
- [ ] Set HSTS max-age: `SECURITY_HSTS_MAX_AGE=31536000`
- [ ] Enable HSTS preload if appropriate: `SECURITY_HSTS_PRELOAD=true`
- [ ] Set CSP mode: `SECURITY_CSP_MODE=strict`
- [ ] Configure frame options: `SECURITY_FRAME_OPTIONS=deny`
- [ ] Set referrer policy: `SECURITY_REFERRER_POLICY=strict-origin-when-cross-origin`
- [ ] Test security headers with security headers checker

### Environment Variables
- [ ] Review all environment variables in `.env`
- [ ] Remove development-specific variables
- [ ] Set production-specific values
- [ ] Use environment variable manager (e.g., systemd, Docker secrets, AWS Secrets Manager)
- [ ] Document required environment variables for operations team

### Dependency Auditing

- [ ] Run `cargo deny check` and `cargo audit` against the generated
  `Cargo.lock` before each release, and re-check when dependencies are
  updated (CI's `deps` job does this for the default render)

**Known advisories in the shipped lockfile.** These are transitive
dependencies with no safe upgrade available yet — track them upstream and
re-check after each `cargo update`:

- `lru` 0.16.4 — RUSTSEC-2026-0253 (unsound: potential use-after-free in
  `LruCache::pop()`). Transitive via `toasty-core`/`toasty-cli`. Reported as
  a warning by `cargo deny`/`cargo audit`; resolution requires a `toasty`
  release that bumps `lru`.
- `mysql_async` 0.37.0 — yanked from crates.io. It is present in the
  lockfile via `toasty-driver-mysql`, but no database option in this
  template enables the MySQL driver, so the crate is never compiled into
  the binary. The warning can be treated as cosmetic.
{% if database == "postgresql" %}- `rustls-pemfile` 2.2.0 — RUSTSEC-2025-0134 (unmaintained, not a known
  vulnerability). Transitive via `toasty-driver-postgresql`, which is only
  included because this project was generated with `database=postgresql`.
  There is no safe upgrade: `toasty` must first migrate to the PEM support
  in `rustls-pki-types`. Until then, `cargo deny check` fails on it. If you
  accept the risk of an unmaintained parser wrapper, silence it in
  `deny.toml`:

  ```toml
  [advisories]
  ignore = [
      # Unmaintained, not vulnerable — transitive via toasty-driver-postgresql.
      # Remove once toasty migrates to rustls-pki-types' PemObject.
      "RUSTSEC-2025-0134",
  ]
  ```
{% endif %}

## Infrastructure

### Server Configuration
- [ ] Set `SERVER_IP=0.0.0.0` to bind to all interfaces
- [ ] Set appropriate `PORT` (e.g., 3000, 8080)
- [ ] Set `DOMAIN_NAME` to production domain
- [ ] Configure firewall rules
- [ ] Set up log rotation

### Reverse Proxy
- [ ] Configure Nginx/Caddy/Traefik as reverse proxy
- [ ] Configure SSL/TLS termination
- [ ] Set up gzip/brotli compression
- [ ] Configure request timeouts
- [ ] Set up proxy headers (X-Forwarded-For, X-Forwarded-Proto)
- [ ] Configure rate limiting at proxy level (optional)

### Storage
- [ ] Configure storage backend for production
- [ ] For local storage: ensure disk space and permissions
- [ ] For S3: configure credentials and bucket
- [ ] Set up CDN if using one
- [ ] Test file upload/download functionality

### Logging
- [ ] Set `RUST_LOG=info` or `warn` for production
- [ ] Configure structured logging output
- [ ] Set up log aggregation (e.g., Loki, ELK, CloudWatch)
- [ ] Configure log retention policy
- [ ] Test log delivery

### Monitoring
- [ ] Set up application monitoring (optional)
- [ ] Configure health check endpoint
- [ ] Set up uptime monitoring
- [ ] Configure alerting for errors
- [ ] Monitor resource usage (CPU, memory, disk)

## Performance

### Database
- [ ] Run database migrations in production
- [ ] Create database indexes for frequently queried fields
- [ ] Analyze query performance
- [ ] Configure connection pool size
- [ ] Set up read replicas if needed (planned feature)

### Caching
- [ ] Consider caching strategy (planned feature)
- [ ] Configure static asset caching headers
- [ ] Set up CDN for static assets (optional)

### Build Optimization
- [ ] Build release binary: `cargo build --release`
- [ ] Enable LTO in Cargo.toml if desired
- [ ] Strip binary to reduce size
- [ ] Test release build locally

## Operations

### Deployment Process
- [ ] Document deployment process
- [ ] Set up CI/CD pipeline (optional)
- [ ] Create rollback procedure
- [ ] Test deployment in staging environment first
- [ ] Plan deployment window

### Backup Strategy
- [ ] Set up automated database backups
- [ ] Test backup restoration
- [ ] Back up uploaded files (if using local storage)
- [ ] Store backups off-site
- [ ] Document backup retention policy

### Disaster Recovery
- [ ] Document disaster recovery procedure
- [ ] Test recovery procedure
- [ ] Identify single points of failure
- [ ] Plan for high availability if needed

## Testing

### Pre-Deployment Testing
- [ ] Run all tests: `cargo test`
- [ ] Run integration tests
- [ ] Test authentication flow
- [ ] Test OAuth callback
- [ ] Test API endpoints
- [ ] Test file upload/download
- [ ] Test rate limiting
- [ ] Load test application

### Smoke Tests
- [ ] Verify health check endpoint responds
- [ ] Test login flow
- [ ] Test creating a resource
- [ ] Test API token creation
- [ ] Verify logs are being generated

## Compliance

### Data Privacy
- [ ] Review data retention policy
- [ ] Implement data deletion if required
- [ ] Review GDPR/CCPA compliance if applicable
- [ ] Document data processing activities

### Accessibility
- [ ] Test with screen readers
- [ ] Verify keyboard navigation
- [ ] Check color contrast
- [ ] Test with accessibility tools

## Post-Deployment

### Verification
- [ ] Verify application is accessible
- [ ] Test critical user flows
- [ ] Check error rates in logs
- [ ] Monitor resource usage
- [ ] Verify backups are running

### Documentation
- [ ] Update deployment documentation
- [ ] Document any production-specific configurations
- [ ] Share operational knowledge with team
- [ ] Update runbooks

## Environment Variables Reference

### Required
- `DATABASE_URL` - PostgreSQL connection string
- `SESSION_KEY` - 64+ byte cryptographically secure key
- `WEB_ALLOWED_ORIGINS` - Comma-separated allowed origins

### Recommended
- `PORT` - Server port (default: 8888)
- `DOMAIN_NAME` - Application domain
{% if oauth_github %}- `GH_CLIENT_ID` - GitHub OAuth client ID
- `GH_CLIENT_SECRET` - GitHub OAuth client secret
- `GH_REDIRECT_URI` - GitHub OAuth callback URL
{% endif %}{% if oauth_google %}- `GOOGLE_CLIENT_ID` - Google OAuth client ID
- `GOOGLE_CLIENT_SECRET` - Google OAuth client secret
- `GOOGLE_REDIRECT_URI` - Google OAuth callback URL
{% endif %}{% if oauth_facebook %}- `FACEBOOK_CLIENT_ID` - Facebook app ID
- `FACEBOOK_CLIENT_SECRET` - Facebook app secret
- `FACEBOOK_REDIRECT_URI` - Facebook OAuth callback URL
{% endif %}

### Optional
- `RUST_LOG` - Log level (default: info)
- `SECURITY_HSTS_ENABLED` - Enable HSTS (default: false)
- `SECURITY_CSP_MODE` - CSP mode (default: strict)
- `RATE_LIMITER_API_REQUEST_RATE_SECONDS` - API rate limit
- `RATE_LIMITER_API_REQUEST_BURST` - API burst limit

## Common Pitfalls

### Don't
- Use SQLite in production
- Commit `.env` to version control
- Use development session key in production
- Forget to set up HTTPS
- Skip testing OAuth callback URL
- Ignore log monitoring
- Forget database backups

### Do
- Use PostgreSQL in production
- Use environment variable manager
- Generate secure session key
- Enable HTTPS
- Test OAuth flow end-to-end
- Monitor logs and errors
- Set up automated backups

## Additional Resources

- [Deployment Guide](DEPLOYMENT.md)
- [Configuration Reference](CONFIGURATION.md)
- [Security Best Practices](MIDDLEWARE.md#security-headers)
- [Rate Limiting Configuration](RATE_LIMITING.md)

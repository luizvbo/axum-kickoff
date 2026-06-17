# Production Deployment Checklist

Use this checklist when deploying axum-kickoff to production.

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
- [ ] Create a production GitHub OAuth application
- [ ] Set production callback URL (HTTPS)
- [ ] Use production GitHub Client ID and Secret
- [ ] Verify redirect URI matches exactly
- [ ] Test OAuth flow in production environment

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
- `GH_CLIENT_ID` - GitHub OAuth client ID
- `GH_CLIENT_SECRET` - GitHub OAuth client secret
- `GH_REDIRECT_URI` - OAuth callback URL

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

# Roadmap

This document outlines the development roadmap for axum-kickoff, including planned features, improvements, and implementation priorities.

## Overview

axum-kickoff is a production-ready Rust web application starter template. This roadmap prioritizes features that provide universal value for web applications while maintaining simplicity and cost-consciousness.

## Philosophy

The roadmap follows these principles:

1. **Universal Value**: Implement features that benefit most web applications
2. **Simplicity First**: Prefer simple solutions over complex ones
3. **Gradual Complexity**: Start simple, upgrade when needed
4. **Cost-Conscious**: Self-hostable with minimal external dependencies
5. **Production-Ready**: Based on battle-tested patterns from crates.io

## Current Status

### Implemented ✅

- **Core Infrastructure**: Axum server with Tokio runtime
- **Database**: Toasty ORM with SQLite support
- **Authentication**: GitHub OAuth, session management, API tokens
- **Middleware Stack**: Security headers, CORS, rate limiting, logging
- **Storage**: Local filesystem storage abstraction
- **Testing**: Integration test infrastructure with snapshot testing
- **Configuration**: Environment-based configuration
- **Error Handling**: Structured error handling with AppError
- **Rate Limiting**: In-memory token bucket algorithm

## Planned Features

### Phase 1: Foundation Enhancements (High Priority)

#### 1. Enhanced Error Handling

**Status**: Partially implemented

**What to Implement**:
- Domain-specific error types with `thiserror`
- Error builders for common HTTP errors
- Enhanced error context with `anyhow::Context`
- User-friendly error messages without sensitive data

**Justification**: Consistent error handling is fundamental to maintainable codebases.

**Effort**: 2-3 days

**Files**:
- `src/util/errors.rs` (enhance existing)

#### 2. Enhanced Configuration Management

**Status**: Partially implemented

**What to Implement**:
- Database pool configuration (pool size, timeouts)
- Read replica support for PostgreSQL
- Connection health checks
- Feature flags system
- Secret handling with `secrecy` crate

**Justification**: Good configuration management prevents environment-specific bugs.

**Effort**: 2-3 days

**Files**:
- `src/config/database.rs` (enhance)
- `src/config/base.rs` (enhance)

#### 3. Controller Organization

**Status**: Basic structure exists

**What to Implement**:
- Domain-based controller organization
- Controller helpers (pagination, sorting, filtering)
- Response helpers for consistent formatting
- Request validation helpers
- Origin verification for sensitive operations

**Justification**: Good controller organization scales well as application grows.

**Effort**: 2-3 days

**Files**:
- `src/controllers/util.rs` (new)
- `src/controllers/*.rs` (reorganize)

#### 4. Database Migration System

**Status**: Toasty handles schema automatically

**What to Implement**:
- Formalize Toasty's schema management
- Create migration scripts for PostgreSQL
- Add migration commands to justfile
- Document migration process

**Justification**: Essential for production schema changes.

**Effort**: 2-3 days

**Files**:
- `migrations/` (new directory)
- `Justfile` (add migration commands)

### Phase 2: Core Features (High Priority)

#### 5. Email System (Feature-Gated)

**Status**: Not implemented

**What to Implement**:
- Email templates with Askama
- SMTP integration via lettre
- File transport for development (emails written to disk)
- Email queue for background processing
- Email types: notifications, confirmations, password reset

**Justification**: Email is essential for user workflows.

**Effort**: 3-4 days

**Files**:
- `src/email.rs` (new)
- `src/email/templates/` (new directory)
- `Cargo.toml` (add lettre dependency)

**Feature Flag**: `email`

#### 6. Background Worker System

**Status**: Not implemented

**What to Implement**:
- Simple job queue (database-backed or in-memory)
- Job runner with Tokio tasks
- Job scheduling (cron-like)
- Job retry with exponential backoff
- Job status tracking

**Justification**: Background jobs improve performance for async tasks.

**Effort**: 5-7 days

**Options**:
- Simple: Tokio tasks and channels
- Complex: PostgreSQL-backed queue

**Recommendation**: Start with simple Tokio tasks.

**Files**:
- `src/jobs.rs` (new)
- `src/bin/worker.rs` (new)

**Feature Flag**: `jobs`

#### 7. Enhanced Metrics

**Status**: Basic metrics exist (feature-gated)

**What to Implement**:
- Service vs instance metrics separation
- Database fallback metrics
- Business metrics (user signups, active users)
- Custom metric helpers

**Justification**: Observability is critical for production.

**Effort**: 2-3 days

**Files**:
- `src/metrics.rs` (enhance)

**Feature Flag**: `metrics` (existing)

#### 8. CORS Middleware Enhancement

**Status**: Basic CORS exists

**What to Implement**:
- Configurable allowed origins from config
- Per-route CORS configuration
- Preflight request handling

**Justification**: Essential for API endpoints.

**Effort**: 1 day

**Files**:
- `src/middleware/cors.rs` (new or enhance existing)

#### 9. Request ID Middleware

**Status**: Not implemented

**What to Implement**:
- Request ID generation
- Include in logging spans
- Add to response headers

**Justification**: Essential for debugging in production.

**Effort**: 1 day

**Files**:
- `src/middleware/request_id.rs` (new)

### Phase 3: Advanced Features (Medium Priority)

#### 10. Storage Abstraction Enhancement

**Status**: Local filesystem implemented

**What to Implement**:
- S3 backend for production
- In-memory backend for testing
- CDN integration
- Presigned URLs for S3

**Justification**: S3 is standard for production file storage.

**Effort**: 3-4 days

**Files**:
- `src/storage.rs` (enhance)
- `Cargo.toml` (add object_store dependency)

**Feature Flag**: `s3`

#### 11. OpenAPI Documentation

**Status**: Not implemented

**What to Implement**:
- Auto-generated OpenAPI spec with utoipa
- API documentation endpoint
- Security schemes documentation
- Internal endpoint marking

**Justification**: Valuable for public APIs but adds overhead.

**Effort**: 3-4 days

**Files**:
- `src/openapi.rs` (new)
- `Cargo.toml` (add utoipa dependencies)

**Feature Flag**: `openapi`

#### 12. Pagination Helpers

**Status**: Not implemented

**What to Implement**:
- Standard pagination (page, per_page)
- Pagination metadata (total count, total pages, next/prev links)
- Cursor-based pagination for large datasets (optional)

**Justification**: Standard requirement for list endpoints.

**Effort**: 2-3 days

**Files**:
- `src/util/pagination.rs` (new)

#### 13. Request Validation

**Status**: Not implemented

**What to Implement**:
- Input validation with validator crate
- Validation error messages
- Validation helpers

**Justification**: Prevents invalid data from reaching business logic.

**Effort**: 2-3 days

**Files**:
- `src/util/validation.rs` (new)
- `Cargo.toml` (add validator dependency)

### Phase 4: Polish (Low Priority)

#### 14. Health Check Endpoint

**Status**: Not implemented

**What to Implement**:
- `/health` endpoint
- Database connectivity check
- External service dependency checks
- Detailed health status

**Justification**: Useful for monitoring and load balancers.

**Effort**: 1-2 days

**Files**:
- `src/controllers/health.rs` (new)

#### 15. Database Read Replicas

**Status**: Not implemented

**What to Implement**:
- Read replica support in database config
- Automatic read replica routing
- Fallback to primary on replica failure
- Replica health monitoring

**Justification**: Improves read performance and reduces load on primary.

**Effort**: 3-4 days

**Files**:
- `src/config/database.rs` (enhance)
- `src/db.rs` (enhance)

#### 16. Webhooks System

**Status**: Not implemented

**What to Implement**:
- Webhook registration
- Webhook delivery with retries
- Webhook signature verification
- Webhook event types

**Justification**: Useful for integrations and notifications.

**Effort**: 4-5 days

**Files**:
- `src/webhooks.rs` (new)
- `src/bin/webhook_worker.rs` (new)

**Feature Flag**: `webhooks`

## Components NOT to Implement

The following components from crates.io are domain-specific and should NOT be implemented:

- **Git index management**: crates.io-specific for package registry
- **Tarball processing**: crates.io-specific for crate uploads
- **Trusted Publishing**: crates.io-specific for CI/CD publishing
- **CDN log processing**: crates.io-specific for download analytics
- **Team/organization management**: crates.io-specific for crate ownership
- **Version downloads tracking**: crates.io-specific analytics
- **Sparse index**: crates.io-specific for cargo sparse protocol
- **GitHub App integration**: crates.io-specific for trusted publishing
- **Database dump generation**: crates.io-specific for public data dumps
- **RSS feeds**: crates.io-specific for crate update feeds
- **Cargo compatibility middleware**: crates.io-specific for cargo CLI
- **Frontend HTML middleware**: crates.io-specific for SvelteKit SPA

## Alternative Approaches

These components have cost-conscious alternatives in axum-kickoff:

- **Sentry error tracking**: Use QuickWit instead (documented in `docs/quickwit-integration.md`)
- **Separate frontend**: Use HTMX + Alpine.js instead of SvelteKit
- **Complex background job system**: Use simple database-backed queue instead of SQS
- **S3-only storage**: Support local filesystem for development/testing
- **PostgreSQL-specific features**: Use SQLite-compatible alternatives where possible

## Implementation Timeline

### Q1 2024

- Enhanced error handling
- Enhanced configuration management
- Controller organization
- Database migration system

### Q2 2024

- Email system (feature-gated)
- Background worker system (feature-gated)
- Enhanced metrics
- CORS middleware enhancement
- Request ID middleware

### Q3 2024

- Storage abstraction enhancement (S3)
- OpenAPI documentation (feature-gated)
- Pagination helpers
- Request validation

### Q4 2024

- Health check endpoint
- Database read replicas
- Webhooks system (feature-gated)

## Contributing

We welcome contributions! See [Development Documentation](DEVELOPMENT.md) for guidelines.

### How to Contribute

1. Check this roadmap for planned features
2. Open an issue to discuss implementation
3. Submit a pull request with your changes
4. Follow the development guidelines

### Feature Requests

To request a new feature:

1. Check if it's already planned in this roadmap
2. Open an issue with a detailed proposal
3. Explain the use case and benefits
4. Consider if it provides universal value

## Dependencies

### Current Dependencies

See `Cargo.toml` for current dependencies.

### Planned Dependencies

- `lettre` - Email sending (feature-gated)
- `object_store` - S3 storage abstraction (feature-gated)
- `utoipa` - OpenAPI documentation (feature-gated)
- `validator` - Request validation
- `secrecy` - Secret handling

## Performance Goals

- **Response Time**: < 100ms for API endpoints
- **Throughput**: 10,000+ requests per second with Redis rate limiting
- **Database**: < 10ms for simple queries
- **Memory**: < 512MB for typical workload
- **Startup Time**: < 5 seconds

## Security Goals

- **Authentication**: Support for OAuth 2.0 and API tokens
- **Authorization**: Fine-grained permissions with scopes
- **Rate Limiting**: Protect against abuse
- **Security Headers**: All recommended headers enabled
- **Input Validation**: Validate all user input
- **Secrets**: Never log secrets, use environment variables

## See Also

- [Architecture Documentation](ARCHITECTURE.md)
- [Development Documentation](DEVELOPMENT.md)
- [Configuration Documentation](CONFIGURATION.md)
- [crates.io Comparison](CRATES_IO_COMPARISON.md) (archived)

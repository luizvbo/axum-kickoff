# Development

This guide covers development workflow, coding standards, and contribution guidelines for axum-kickoff.

## Development Environment

### Prerequisites

- Rust 1.70 or later
- Git
- SQLite (for development)
- PostgreSQL (optional, for testing with PostgreSQL)

### Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/luizvbo/axum-kickoff.git
   cd axum-kickoff
   ```

2. Install dependencies:
   ```bash
   cargo build
   ```

3. Set up environment:
   ```bash
   cp .env.sample .env
   # Edit .env with your configuration
   ```

4. Run the server:
   ```bash
   cargo run --bin server
   ```

## Project Structure

```
axum-kickoff/
├── src/
│   ├── bin/              # Binary entry points
│   │   └── server.rs     # Main server binary
│   ├── controllers/      # HTTP request handlers
│   │   ├── auth.rs      # Authentication endpoints
│   │   └── token.rs     # API token management
│   ├── middleware/       # Axum middleware
│   │   ├── mod.rs       # Middleware stack orchestration
│   │   ├── session.rs   # Session management
│   │   ├── api_token.rs # API token authentication
│   │   ├── security_headers.rs # Security headers
│   │   └── ...          # Other middleware
│   ├── models/          # Database models (Toasty)
│   │   ├── user.rs      # User model
│   │   ├── token.rs     # API token model
│   │   └── oauth_github.rs # GitHub OAuth model
│   ├── config/          # Configuration management
│   │   ├── mod.rs       # Configuration module
│   │   ├── base.rs      # Base configuration
│   │   └── database.rs  # Database configuration
│   ├── util/            # Utility functions
│   │   ├── auth.rs      # Authentication utilities
│   │   ├── errors.rs    # Error handling
│   │   └── ...          # Other utilities
│   ├── tests/           # Integration test infrastructure
│   │   ├── test_app.rs  # Test application builder
│   │   ├── request_helper.rs # HTTP request helpers
│   │   ├── response.rs  # Response wrapper
│   │   └── builders.rs  # Test data builders
│   ├── app.rs           # Application state
│   ├── db.rs            # Database connection
│   ├── rate_limiter.rs  # Rate limiting
│   ├── storage.rs       # Storage abstraction
│   ├── router.rs        # Router configuration
│   └── lib.rs           # Library entry point
├── templates/           # Askama templates
├── static/              # Static assets (CSS, JS)
├── docs/                # Documentation
├── Cargo.toml           # Rust dependencies
└── README.md            # Project README
```

## Coding Standards

### Rust Style

Follow the official Rust style guidelines:

- Use `cargo fmt` to format code
- Use `cargo clippy` for linting
- Follow naming conventions (snake_case for functions/variables, PascalCase for types)

### Error Handling

Use the established error handling patterns:

```rust
use crate::util::errors::{AppError, AppResult};

pub async fn my_handler() -> AppResult<Json<Response>> {
    // Use ? operator for error propagation
    let data = fetch_data().await?;

    // Use anyhow::Context for error context
    let result = process_data(&data)
        .context("Failed to process data")?;

    Ok(Json(result))
}
```

### Controller Pattern

Controllers should:

1. Extract request data
2. Validate input
3. Call business logic
4. Return appropriate responses

```rust
use axum::{Json, extract::State};
use crate::util::errors::AppResult;

pub async fn create_post(
    State(app): State<AppState>,
    Json(input): Json<CreatePostInput>,
) -> AppResult<Json<PostResponse>> {
    // Validate input
    input.validate()?;

    // Create post
    let post = Post::create(&app.db, input).await?;

    // Return response
    Ok(Json(post.into_response()))
}
```

### Database Models

Use Toasty ORM for database models:

```rust
use toasty::Model;

#[derive(Model, Debug)]
pub struct User {
    #[pk]
    pub id: Id<User>,

    pub github_id: i64,
    pub github_login: String,
    pub avatar_url: String,

    pub created_at: DateTime,
}
```

### Documentation

Document public APIs with Rustdoc:

```rust
/// Creates a new user with the given GitHub information.
///
/// # Arguments
///
/// * `db` - Database connection
/// * `github_id` - GitHub user ID
/// * `github_login` - GitHub username
///
/// # Returns
///
/// Returns the created user or an error if creation fails.
///
/// # Errors
///
/// Returns an error if:
/// - Database connection fails
/// - User already exists
pub async fn create_user(
    db: &Database,
    github_id: i64,
    github_login: String,
) -> AppResult<User> {
    // Implementation
}
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run integration tests only
cargo test --test '*'

# Accept snapshot changes
cargo insta accept
```

### Writing Tests

Use the test infrastructure in `src/tests/`:

```rust
use axum_kickoff::tests::{TestApp, AnonymousUser};
use http::StatusCode;

#[tokio::test]
async fn test_health_check() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(StatusCode::OK);
}
```

See [Testing Documentation](TESTING.md) for detailed testing guidelines.

## Database Migrations

### Toasty Schema

Database models are defined in `src/models/` using Toasty macros. The schema is managed automatically by Toasty.

### Generating Models

```bash
# Generate models from schema
cargo run --bin toasty
```

### Manual Database Changes

For manual database changes:

1. Update model definitions in `src/models/`
2. Run Toasty to regenerate
3. Test with SQLite in-memory database
4. Document migration steps for PostgreSQL

## Adding New Features

### Adding a New Controller

1. Create controller file in `src/controllers/`
2. Implement handler functions
3. Add routes in `src/router.rs`
4. Add tests in `src/tests/`
5. Update documentation

### Adding New Middleware

1. Create middleware file in `src/middleware/`
2. Implement middleware function
3. Add to middleware stack in `src/middleware/mod.rs`
4. Add tests
5. Update documentation

### Adding New Model

1. Create model file in `src/models/`
2. Define model with Toasty macros
3. Run Toasty to regenerate
4. Add CRUD operations
5. Add tests
6. Update documentation

## Git Workflow

### Branching

- `main` - Production branch
- `develop` - Development branch
- `feature/*` - Feature branches
- `bugfix/*` - Bug fix branches

### Commit Messages

Follow conventional commits:

```
feat: add user registration
fix: resolve session cookie issue
docs: update authentication documentation
test: add integration tests for API
refactor: simplify error handling
```

### Pull Requests

1. Create feature branch from `develop`
2. Make changes with clear commit messages
3. Add tests for new features
4. Update documentation
5. Submit PR to `develop`
6. Request code review

## Code Review

### Review Checklist

- [ ] Code follows style guidelines
- [ ] Tests are included and passing
- [ ] Documentation is updated
- [ ] Error handling is proper
- [ ] Security considerations are addressed
- [ ] No sensitive data in logs
- [ ] Performance considerations are addressed

## Performance

### Profiling

Use profiling tools to identify bottlenecks:

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin server
```

### Database Optimization

- Use indexes on frequently queried fields
- Use connection pooling
- Consider read replicas for high read volume
- Use prepared statements (handled by Toasty)

### Caching

Consider caching for:
- Frequently accessed data
- Expensive computations
- External API responses

## Security

### Security Checklist

- [ ] Input validation on all endpoints
- [ ] SQL injection prevention (use Toasty ORM)
- [ ] XSS prevention (use CSP and template escaping)
- [ ] CSRF protection (use SameSite cookies)
- [ ] Rate limiting on sensitive endpoints
- [ ] Authentication and authorization checks
- [ ] No sensitive data in logs
- [ ] Secure session management
- [ ] Secure password handling (if applicable)
- [ ] Dependency updates

### Security Audits

Regularly:

- Update dependencies: `cargo update`
- Check for vulnerabilities: `cargo audit`
- Review security headers
- Test authentication flows

## Debugging

### Logging

Use structured logging with tracing:

```rust
use tracing::{info, error, debug, instrument};

#[instrument(skip(app))]
pub async fn my_handler(State(app): State<AppState>) -> AppResult<()> {
    info!("Starting handler");
    debug!("Processing request");

    match do_something().await {
        Ok(result) => {
            info!("Request completed successfully");
            Ok(result)
        }
        Err(e) => {
            error!("Request failed: {:?}", e);
            Err(e.into())
        }
    }
}
```

### Debug Builds

```bash
# Build with debug symbols
cargo build

# Run with debug logging
RUST_LOG=debug cargo run --bin server
```

### Common Issues

**Database Connection Failed:**
- Check DATABASE_URL
- Verify database is running
- Check credentials

**Compilation Errors:**
- Run `cargo clean` and rebuild
- Check Rust version
- Verify dependencies

**Test Failures:**
- Check test database configuration
- Verify test data setup
- Check for race conditions

## Continuous Integration

The project uses GitHub Actions for CI. See `.github/workflows/ci.yml` for configuration.

### CI Pipeline

1. Run tests on all supported Rust versions
2. Run clippy for linting
3. Check code formatting
4. Run security audit
5. Build release binary

## Release Process

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Create git tag
4. Push to GitHub
5. Create GitHub release

## Contributing

### Before Contributing

1. Read this development guide
2. Read the architecture documentation
3. Set up development environment
4. Run existing tests
5. Review open issues

### Making Contributions

1. Fork the repository
2. Create feature branch
3. Make changes with tests
4. Update documentation
5. Submit pull request

### Contribution Guidelines

- Keep changes focused and minimal
- Add tests for new features
- Update relevant documentation
- Follow coding standards
- Write clear commit messages
- Be responsive to code review feedback

## Resources

- [Rust Documentation](https://doc.rust-lang.org/)
- [Axum Documentation](https://docs.rs/axum/)
- [Toasty Documentation](https://github.com/stepchowfun/toasty)
- [Tokio Documentation](https://tokio.rs/)
- [Tracing Documentation](https://docs.rs/tracing/)

## See Also

- [Getting Started Guide](GETTING_STARTED.md)
- [Architecture Documentation](ARCHITECTURE.md)
- [Testing Documentation](TESTING.md)
- [Configuration Documentation](CONFIGURATION.md)

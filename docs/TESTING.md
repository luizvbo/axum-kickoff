# Testing

This document covers testing strategies, conventions, and infrastructure for axum-kickoff.

## Overview

axum-kickoff uses a comprehensive testing approach including:

- **Unit Tests**: Test individual functions and modules
- **Integration Tests**: Test HTTP endpoints and database interactions
- **Snapshot Testing**: Validate API responses with `insta`
- **Test Infrastructure**: Reusable test utilities and helpers

## Test Infrastructure

The test infrastructure is located in `src/tests/` and provides:

### TestApp

Creates a test-ready application with isolated database:

```rust
use axum_kickoff::tests::TestApp;

#[tokio::test]
async fn test_example() {
    let app = TestApp::new();
    // Use app for testing
}
```

**Features:**
- Isolated SQLite database (temporary file)
- Test configuration with minimal settings
- Exposes router, state, and database
- Automatic cleanup on drop

### RequestHelper Trait

Trait for making HTTP requests in tests:

```rust
use axum_kickoff::tests::{TestApp, AnonymousUser};

#[tokio::test]
async fn test_get_endpoint() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/endpoint").await;
    response.assert_status(StatusCode::OK);
}
```

**Authentication States:**
- `AnonymousUser` - No authentication
- `CookieUser` - Session cookie authentication
- `TokenUser` - API token authentication

**Methods:**
- `get`, `post`, `put`, `delete`, `patch` - HTTP methods
- Automatic request building and headers

### Response Wrapper

Wrapper around axum responses with helper methods:

```rust
use axum_kickoff::tests::response::TestResponse;

let response = anon.get::<()>("/api/endpoint").await;

// Status assertions
response.assert_status(StatusCode::OK);
response.assert_success();

// Body extraction
let body: String = response.into_string();
let json: MyType = response.into_json();

// Content-type
let content_type = response.content_type();
```

### Test Builders

Builder pattern for creating test data:

```rust
use axum_kickoff::tests::builders::UserBuilder;

let user = UserBuilder::new()
    .with_github_login("testuser")
    .with_avatar_url("https://example.com/avatar.png")
    .build(&app.db)
    .await;
```

## Running Tests

### All Tests

```bash
cargo test
```

### Unit Tests Only

```bash
cargo test --lib
```

### Integration Tests Only

```bash
cargo test --test '*'
```

### Specific Test

```bash
cargo test test_name
```

### With Output

```bash
cargo test -- --nocapture
```

### Snapshot Testing

```bash
# Run tests (will fail if snapshots don't match)
cargo test

# Review snapshot changes
cargo insta review

# Accept all snapshot changes
cargo insta accept
```

## Writing Tests

### Unit Tests

Unit tests test individual functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        let result = my_function("input");
        assert_eq!(result, "expected");
    }
}
```

### Integration Tests

Integration tests test HTTP endpoints:

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

### Authenticated Tests

Test with authenticated user:

```rust
use axum_kickoff::tests::{TestApp, CookieUser};

#[tokio::test]
async fn test_protected_endpoint() {
    let app = TestApp::new();
    let user = CookieUser::new(app, "test_user").await;

    let response = user.get::<()>("/api/protected").await;
    response.assert_success();
}
```

### Snapshot Tests

Snapshot tests validate API responses:

```rust
use axum_kickoff::tests::{TestApp, AnonymousUser};
use insta::assert_json_snapshot;

#[tokio::test]
async fn test_api_response() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/api/data").await;
    assert_json_snapshot!(response.into_json());
}
```

## Test Conventions

### File Organization

- Unit tests in same file as code (in `#[cfg(test)]` module)
- Integration tests in `src/tests/`
- Test utilities in `src/tests/`
- Snapshots in `snapshots/` directory

### Naming

- Test functions: `test_<what_is_being_tested>`
- Test modules: `tests` (in code), or descriptive names (in `src/tests/`)
- Snapshot files: `<module>__<test_name>.snap`

### Structure

```rust
#[tokio::test]
async fn test_endpoint_name() {
    // Arrange
    let app = TestApp::new();
    let user = CookieUser::new(app, "user").await;

    // Act
    let response = user.get::<()>("/api/endpoint").await;

    // Assert
    response.assert_status(StatusCode::OK);
}
```

## Test Database

### Configuration

Tests use in-memory SQLite by default:

```bash
TEST_DATABASE_URL=sqlite::memory:
```

### Isolation

Each test gets an isolated database:
- Temporary file created on `TestApp::new()`
- Cleaned up on `TestApp` drop
- No shared state between tests

### Test Data

Use builders to create test data:

```rust
let user = UserBuilder::new()
    .with_github_login("testuser")
    .build(&app.db)
    .await;
```

## Common Test Patterns

### Testing CRUD Operations

```rust
#[tokio::test]
async fn test_create_post() {
    let app = TestApp::new();
    let user = CookieUser::new(app, "user").await;

    // Create
    let input = CreatePostInput { title: "Test".to_string() };
    let response = user.post("/api/posts", &input).await;
    response.assert_status(StatusCode::CREATED);

    // Read
    let post: Post = response.into_json();
    assert_eq!(post.title, "Test");
}
```

### Testing Error Cases

```rust
#[tokio::test]
async fn test_invalid_input() {
    let app = TestApp::new();
    let user = CookieUser::new(app, "user").await;

    let invalid_input = CreatePostInput { title: "".to_string() };
    let response = user.post("/api/posts", &invalid_input).await;
    response.assert_status(StatusCode::BAD_REQUEST);
}
```

### Testing Authentication

```rust
#[tokio::test]
async fn test_unauthorized_access() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/protected").await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}
```

### Testing Rate Limiting

```rust
#[tokio::test]
async fn test_rate_limiting() {
    let app = TestApp::new();
    let anon = AnonymousUser::new(app);

    // Make multiple requests
    for _ in 0..100 {
        let response = anon.get::<()>("/api/endpoint").await;
        // Eventually should hit rate limit
    }
}
```

## Mocking

### External Services

For external services (GitHub OAuth, S3, etc.):

1. Use feature flags to skip external calls in tests
2. Mock responses at the HTTP level
3. Use test-specific configuration

Example with test configuration:

```rust
#[cfg(test)]
impl TestApp {
    pub fn with_mock_github() -> Self {
        // Configure mock GitHub responses
        Self::new()
    }
}
```

## Test Coverage

### Checking Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html
```

### Coverage Goals

- Aim for >80% coverage on critical paths
- 100% coverage on authentication and authorization
- High coverage on error handling paths

## Continuous Integration

Tests run automatically on GitHub Actions. See `.github/workflows/ci.yml`.

### CI Test Matrix

- Multiple Rust versions
- All test suites
- Clippy linting
- Code formatting check

## Troubleshooting

### Test Database Lock

If tests fail with database lock:

```bash
# Clean up test databases
rm -f /tmp/axum_kickoff_test_*.db
```

### Snapshot Mismatches

If snapshots don't match:

```bash
# Review changes
cargo insta review

# Accept if changes are expected
cargo insta accept
```

### Flaky Tests

If tests are flaky:

- Check for race conditions
- Add delays or synchronization
- Ensure proper cleanup between tests
- Check for shared state

### Memory Leaks in Tests

If tests leak memory:

- Check for unclosed connections
- Ensure proper cleanup in test teardown
- Use `--test-threads=1` to isolate tests

## Best Practices

### DO

- Write tests for all public APIs
- Test both success and error paths
- Use descriptive test names
- Keep tests focused and simple
- Use test builders for data setup
- Clean up resources in tests
- Use snapshot testing for API responses

### DON'T

- Don't test implementation details
- Don't rely on external services in tests
- Don't share state between tests
- Don't ignore flaky tests
- Don't write overly complex tests
- Don't hardcode test data values

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Axum Testing Guide](https://docs.rs/axum/latest/axum/#testing)
- [Insta Documentation](https://insta.rs/)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)

## See Also

- [Development Documentation](DEVELOPMENT.md)
- [Architecture Documentation](ARCHITECTURE.md)

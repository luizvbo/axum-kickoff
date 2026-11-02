# Testing

This document covers testing strategies, conventions, and infrastructure for {{project-name}}.

## Overview

{{project-name}} uses a comprehensive testing approach including:

- **Unit Tests**: Test individual functions and modules
- **Integration Tests**: Test HTTP endpoints and database interactions
- **Snapshot Testing**: Validate API responses with `insta`
- **Test Infrastructure**: Reusable test utilities and helpers

## Test Infrastructure

The test infrastructure is located in `src/tests/` and provides:

### TestApp

Creates a test-ready application with isolated database:

```rust
use {{crate_name}}::tests::TestApp;

#[tokio::test]
async fn test_example() {
    let app = TestApp::new().await;
    // Use app for testing
}
```

**Features:**
- Isolated SQLite database (temporary file, schema built via `Db::push_schema`)
- Test configuration with minimal settings
- Exposes router, state (`app.state`), and database (`app.db()`)
- Automatic cleanup on drop

### RequestHelper Trait

Trait for making HTTP requests in tests:

```rust
use {{crate_name}}::tests::{TestApp, AnonymousUser};

#[tokio::test]
async fn test_get_endpoint() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(StatusCode::OK);
}
```

**Authentication States:**
- `AnonymousUser::new(app)` - No authentication
- `CookieUser::new(app, user_id, session_key)` - Session cookie auth;
  `session_key` comes from `app.state.session_key`
- `TokenUser::new(app, plain_token)` - API token auth; `plain_token` comes
  from `app.token_builder(...)` (returns `(ApiToken, String)`)

**Methods:**
- `get`, `post`, `put`, `delete`, `patch` - JSON-bodied requests
- `post_with_headers`, `patch_with_headers`, `delete_with_headers` - with
  extra headers (e.g. `CookieUser::headers_with_csrf`)

**CSRF in tests:** a cookie-authenticated session has a `user_id`, so
unsafe methods (POST/PUT/PATCH/DELETE) require a CSRF token. Mint one with
`cookie_user.init_csrf().await` and send it via
`cookie_user.headers_with_csrf(&csrf_token)` + `post_with_headers`.

### Response Wrapper

Wrapper around axum responses with helper methods:

```rust
use {{crate_name}}::tests::Response;

let response = anon.get::<()>("/api/endpoint").await;

// Status assertions
response.assert_status(StatusCode::OK);
response.assert_success();

// Body extraction (async — consumes the response)
let body: String = response.into_string().await;
let json: MyType = response.into_json().await;

// Content-type
let content_type = response.content_type();
```

### Test Builders

Builder pattern for creating test data:

```rust
use {{crate_name}}::tests::builders::UserBuilder;

let user = app
    .user_builder("testuser")
    .name("Test User")
    .build(&mut app.db().db_clone())
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

All tests live inside the library crate (`src/tests/`), so there is no
separate `tests/` target — `cargo test --lib` covers everything. To run
only the `tests` module:

```bash
cargo test tests::
# or with nextest (as CI does):
cargo nextest run -E 'test_id_pattern=~tests::'
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
use {{crate_name}}::tests::{TestApp, AnonymousUser};
use http::StatusCode;

#[tokio::test]
async fn test_health_check() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/health").await;
    response.assert_status(StatusCode::OK);
}
```

### Authenticated Tests

Test with an authenticated user — build the user first, then construct a
`CookieUser` with the app's session key:

```rust
use {{crate_name}}::tests::{TestApp, CookieUser};

#[tokio::test]
async fn test_protected_endpoint() {
    let app = TestApp::new().await;
    let user = app
        .user_builder("test_user")
        .build(&mut app.db().db_clone())
        .await
        .unwrap();
    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    let response = cookie_user.get::<()>("/api/v1/posts").await;
    response.assert_success();
}
```

### Snapshot Tests

Snapshot tests validate API responses:

```rust
use {{crate_name}}::tests::{TestApp, AnonymousUser};
use insta::assert_json_snapshot;

#[tokio::test]
async fn test_api_response() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<serde_json::Value>("/health").await;
    assert_json_snapshot!(response.into_json().await);
}
```

## Test Conventions

### File Organization

- Unit tests in same file as code (in `#[cfg(test)]` module)
- Integration tests in `src/tests/`
- Test utilities in `src/tests/`
- Snapshots in `src/tests/snapshots/` directory

### Naming

- Test functions: `test_<what_is_being_tested>`
- Test modules: `tests` (in code), or descriptive names (in `src/tests/`)
- Snapshot files: `<module>__<test_name>.snap`

### Structure

```rust
#[tokio::test]
async fn test_endpoint_name() {
    // Arrange
    let app = TestApp::new().await;
    let user = app
        .user_builder("user")
        .build(&mut app.db().db_clone())
        .await
        .unwrap();
    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);

    // Act
    let response = cookie_user.get::<()>("/api/endpoint").await;

    // Assert
    response.assert_status(StatusCode::OK);
}
```

## Test Database

### Configuration

Tests use temporary SQLite files by default — no env var needed. To run
them against a different backend (e.g. a scratch Postgres database), set
`TEST_DATABASE_URL`:

```bash
TEST_DATABASE_URL=postgresql://localhost/test_db cargo test
```

`DATABASE_URL` itself is ignored by tests.

### Isolation

Each test gets an isolated database:
- Temporary file created on `TestApp::new()`
- Cleaned up on `TestApp` drop
- No shared state between tests

### Test Data

Use builders to create test data:

```rust
let user = app
    .user_builder("testuser")
    .build(&mut app.db().db_clone())
    .await;
```

## Common Test Patterns

### Testing CRUD Operations

```rust
#[tokio::test]
async fn test_create_post() {
    let app = TestApp::new().await;
    let user = app
        .user_builder("user")
        .build(&mut app.db().db_clone())
        .await
        .unwrap();
    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    // Create
    let input = serde_json::json!({ "title": "Test", "content": "Body" });
    let response = cookie_user
        .post_with_headers::<serde_json::Value>(
            "/api/v1/posts",
            &input,
            cookie_user.headers_with_csrf(&csrf_token),
        )
        .await;
    response.assert_status(StatusCode::CREATED);
}
```

### Testing Error Cases

```rust
#[tokio::test]
async fn test_invalid_input() {
    let app = TestApp::new().await;
    let user = app
        .user_builder("user")
        .build(&mut app.db().db_clone())
        .await
        .unwrap();
    let session_key = app.state.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    let invalid_input = serde_json::json!({ "title": "" });
    let response = cookie_user
        .post_with_headers::<serde_json::Value>(
            "/api/v1/posts",
            &invalid_input,
            cookie_user.headers_with_csrf(&csrf_token),
        )
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}
```

### Testing Authentication

```rust
#[tokio::test]
async fn test_unauthorized_access() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    let response = anon.get::<()>("/api/v1/tokens").await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}
```

### Testing Rate Limiting

```rust
#[tokio::test]
async fn test_rate_limiting() {
    let app = TestApp::new().await;
    let anon = AnonymousUser::new(app);

    // Exceed the per-action budget; a 429 carries a Retry-After header
    for _ in 0..200 {
        let response = anon.get::<()>("/api/v1/posts").await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            assert!(response.headers().contains_key("retry-after"));
            return;
        }
    }
}
```

## Mocking

### External Services

For external services (OAuth providers, S3, etc.):

1. Use feature flags to skip external calls in tests
2. Mock responses at the HTTP level
3. Use test-specific configuration

Example with test configuration:

```rust
#[cfg(test)]
impl TestApp {
    pub fn with_mock_oauth() -> Self {
        // Configure mock OAuth provider responses
        Self::new()
    }
}
```

## Test Coverage

### Checking Coverage

```bash
# Install llvm-cov + nextest
cargo install cargo-llvm-cov cargo-nextest

# Generate coverage report (same tooling as the justfile/CI uses)
cargo llvm-cov nextest
```

### Coverage Goals

- Aim for >80% coverage on critical paths
- 100% coverage on authentication and authorization
- High coverage on error handling paths

## Continuous Integration

Tests run automatically on GitHub Actions. See `.github/workflows/ci.yml`.

### CI Jobs

- `cargo fmt --check` / `cargo clippy --all-targets --all-features` / `cargo doc`
- `cargo insta test --all-features --test-runner nextest`
- `cargo deny check`, `cargo machete`, `cargo audit`, `zizmor`

## Troubleshooting

### Test Database Lock

If tests fail with database lock:

```bash
# Clean up test databases
rm -f /tmp/{{crate_name}}_test_*.db
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

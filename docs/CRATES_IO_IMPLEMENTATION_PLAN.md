# Axum Kickoff — crates.io Best-Practices Implementation Plan

> v1.1 — incorporates the review and design decisions.

This document turns the gap analysis into a set of self-contained GenAI prompts. Each major task is a prompt you can paste into a coding assistant. Run the verification commands after every task.

## How to use this document

1. Read **Project Context** and **Implementation Rules** once.
2. Pick a task, copy the **Prompt** block, and hand it to the assistant.
3. After the assistant finishes, run:
   ```bash
   cargo check --all-features
   cargo clippy --all-targets --all-features
   cargo test --all-features
   ```
4. Move to the next task in order. Phases are designed to leave the repo in a working state after each one.

## Project Context

- Stack: **Axum 0.8**, **Tower 0.5**, **tower-http 0.7**, **Toasty 0.9**, **Askama 0.16**, **HTMX**, **Alpine.js**.
- Frontend is server-rendered HTML. JSON API endpoints live under `/api/v1/`.
- Dev database is **SQLite**. Production can use **SQLite or PostgreSQL**; the base code should support both, but PostgreSQL-specific optimizations are optional add-ons.
- Crates.io-specific patterns (read replicas, `NOTIFY/LISTEN`, `FOR UPDATE SKIP LOCKED`, S3/object storage) are advanced features. They are documented as optional, not required for the base template.
- Existing good stuff: middleware scaffolding, GitHub OAuth, signed sessions, CSRF, scoped API tokens, local storage, Toasty rate-limit bucket, `insta`, CI.
- Source report: `/tmp/tmp_luiz/crates.io/rust-web-template-best-practices-report.md`.

## Implementation Rules

- Keep the HTMX/Askama/Toasty stack. Do not introduce a JS SPA.
- Do not add or remove comments unless the task asks for documentation.
- Write compact, idiomatic Rust. Reuse `App`, `AppState`, `BoxedAppError`, `AppResult`, `HtmlTemplate`, `TestApp`.
- Use `secrecy::SecretString` for every new secret, token, or database URL.
- Read environment variables through `dotenvy` helpers, not `std::env::var`.
- Match existing test style: `tokio::test`, `insta` snapshots, builders, `TestApp`.
- Keep CI green under `RUSTFLAGS='-D warnings'`.
- Feature-gate optional services (`sentry`, `metrics`, `jemalloc`, `postgresql`) like the current `metrics` feature.
- Skip crates.io-specific features: CDN invalidation, Cargo API compat, docs.rs, git index, GitHub team sync.

---

## Phase 1: Foundation — Process, Config, DB, Security Headers

### 1.1 Consolidate to a single binary with subcommands

**Why:** crates.io ships one artifact for `server`, `background-worker`, and `migrate` so there is no version skew. The repo currently has two separate binaries (`server` and `toasty`).

**Relevant files:**
- `src/bin/server.rs`
- `src/bin/cli.rs`
- `Cargo.toml`
- `justfile`
- `.github/workflows/ci.yml`

**Prompt:**

> Implement a single `src/bin/main.rs` using `clap` with subcommands `server`, `background-worker`, and `migrate`. Do **not** add a `monitor` placeholder unless you explicitly need it later.
>
> 1. Move the server startup logic from `src/bin/server.rs` into a `run_server()` async function called by the `server` subcommand.
> 2. Move the Toasty CLI logic from `src/bin/cli.rs` into a `run_migrate()` function that parses the remaining args and calls `ToastyCli::with_config(db, config).parse_and_run()`. By default `cargo run -- migrate` should run `migration apply`.
> 3. Add a `background-worker` subcommand that currently prints a placeholder and exits successfully (real worker is Phase 4).
> 4. Update `Cargo.toml` to have one `[[bin]]` named `axum-kickoff` pointing at `src/bin/main.rs`. Remove the old `server` and `toasty` binaries.
> 5. Add a root `Procfile`:
>    ```
>    web: ./axum-kickoff server
>    background_worker: ./axum-kickoff background-worker
>    release: ./axum-kickoff migrate
>    ```
> 6. Add `script/release.sh` that runs `./axum-kickoff migrate` then `exec ./axum-kickoff server`.
> 7. Update the `justfile`: change `cargo run --bin server` to `cargo run --bin axum-kickoff -- server`, and all `cargo run --bin toasty -- ...` to `cargo run --bin axum-kickoff -- migrate ...`.
>
> Ensure `cargo run -- server` still works and `cargo build --release` produces one `axum-kickoff` binary.

**Acceptance criteria:**
- `cargo run -- server` starts the server.
- `cargo run -- migrate` applies pending migrations.
- `cargo run -- background-worker` exits 0 (placeholder).
- `Procfile`, `script/release.sh`, and `justfile` are updated.
- Only one `[[bin]]` remains in `Cargo.toml`.

---

### 1.2 Centralize configuration and secret handling

**Why:** crates.io has one typed `Server` config loaded through small env helpers. This repo mixes `std::env::var` and `dotenvy::var`, and several secrets live as plain strings.

**Relevant files:**
- `src/config/mod.rs`
- `src/config/server.rs`
- `src/config/base.rs`
- `src/config/database.rs`

**Prompt:**

> Refactor configuration loading to match crates.io patterns.
>
> 1. Create `src/config/env.rs` with helpers:
>    ```rust
>    pub fn var(key: &str) -> anyhow::Result<Option<String>>;
>    pub fn required_var(key: &str) -> anyhow::Result<String>;
>    pub fn var_parsed<R: FromStr>(key: &str) -> anyhow::Result<Option<R>>;
>    ```
>    Wrap `dotenvy::var` and include the key name in `anyhow` context.
> 2. Convert `DatabaseConfig::url` to `secrecy::SecretString`. Use `ExposeSecret` only when opening the connection.
> 3. Convert `SESSION_KEY` handling to `SecretString`; derive the `cookie::Key` with `cookie::Key::derive_from(session_key.expose_secret().as_bytes())`.
> 4. Replace all `std::env::var` and `dotenvy::var` calls in `config/` with the new helpers.
> 5. Add `METRICS_TOKEN` (optional) and `SENTRY_DSN` (optional) to `Server`.
> 6. Add `APP_ENV` detection (`development`/`test`/`production`) in addition to the `HEROKU` fallback. Drive HSTS, TLS, log format, and Sentry from this.
>
> Make no behavioral changes except the ones above. All tests must still pass.

**Acceptance criteria:**
- `src/config/env.rs` exists and is used everywhere in `config/`.
- `DatabaseConfig::url`, `SESSION_KEY`, and `SENTRY_DSN` are `SecretString`.
- `cookie::Key` is derived from the secret.
- `cargo check --all-features` and `cargo test --all-features` pass.

---

### 1.3 Harden the database layer

**Why:** A generic web app needs explicit migrations, safe defaults, and the ability to run on SQLite locally and PostgreSQL in production. The repo currently auto-applies schema and is only compiled for SQLite.

**Relevant files:**
- `src/db.rs`
- `src/config/database.rs`
- `src/app.rs`
- `Cargo.toml`
- `src/tests/test_app.rs`

**Prompt:**

> Harden the database layer for the base template.
>
> 1. Add the `postgresql` feature to `toasty` in `Cargo.toml` so the same binary can connect to PostgreSQL. Make it a Cargo feature of this crate (`postgresql`) that is off by default and toggles the `toasty/postgresql` dependency.
> 2. Remove `db.push_schema().await?` from `Database::from_config`. The server must **not** create/alter schema on startup. Add a `Database::migrate()` async method that runs the Toasty migration apply logic, and use that in the `migrate` subcommand.
> 3. Update `TestApp` (and any other test setup) to call `Database::migrate()` after `Database::from_config()` so the schema is present in tests.
> 4. Set per-connection parameters via the database URL query string, not via a post-checkout hook (Toasty does not expose one):
>    - PostgreSQL: append `?application_name=axum_kickoff&options=-c%20statement_timeout%3D30s` to `DATABASE_URL`.
>    - SQLite: append `?busy_timeout=5000` to the URL.
>    `DatabaseConfig` should expose `connect_url()` that returns a `SecretString` with these defaults merged in; for PostgreSQL, `statement_timeout` and `application_name` should be overridable via env. Use `ExposeSecret` only when passing it to `Db::builder().connect(...)`.
> 5. Parse `sslmode` from `DATABASE_URL` or add `DATABASE_SSLMODE`; default to `require` in production.
> 6. Add `db_read()` and `db_write()` methods on `App`. For now both return the primary pool.
>
> For read-replica support and detailed pool metrics, see `docs/OPTIONAL_ADDONS.md`.
>
> Do not break the in-memory SQLite test setup.

**Acceptance criteria:**
- Server no longer creates/updates schema on startup.
- `cargo run -- migrate` applies pending migrations.
- PostgreSQL feature compiles when enabled.
- `App::db_read()` and `App::db_write()` exist.
- Existing tests pass.

---

### 1.4 Fix security headers for HTMX/Alpine

**Why:** The current headers deviate from crates.io: `X-XSS-Protection` is wrong, HSTS is off, CSP allows `unpkg.com`, and dynamic HTML has no `Cache-Control`.

**Relevant files:**
- `src/middleware/security_headers.rs`
- `src/router.rs`

**Prompt:**

> Fix the security headers middleware.
>
> 1. Change `X-XSS-Protection` to `0`.
> 2. Default `hsts_enabled` to `true` in `Production` / `APP_ENV=production`; keep it configurable via `SECURITY_HSTS_ENABLED`.
> 3. Remove `https://unpkg.com` from the default `script-src`; since HTMX and Alpine are vendored locally, use `script-src 'self' 'nonce-{nonce}'`.
> 4. Add `Cache-Control: no-store, private` for dynamic HTML responses. Do not override `Cache-Control` on `/static/` responses.
> 5. Ensure `CspNonce` is passed to templates. Update `HtmlTemplate` in `src/router.rs` to extract `CspNonce` from request extensions and make `csp_nonce` available to Askama. (The templates will use it in Task 3.4.)
> 6. Keep `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`, and `Permissions-Policy` as-is.
>
> Do not modify templates yet; that is Phase 3.

**Acceptance criteria:**
- `X-XSS-Protection` is `0`.
- Production HSTS is enabled by default.
- CSP strict mode has no `unpkg.com` and includes the nonce.
- Dynamic responses get `Cache-Control: no-store, private`.
- `csp_nonce` is available to all templates rendered through `HtmlTemplate`.
- Tests in `src/middleware/security_headers.rs` are updated.

---

## Phase 2: Middleware, Observability, and Request Handling

### 2.1 Reorder the middleware stack and add Sentry

**Why:** crates.io orders middleware by failure domain. The repo has the pieces in the wrong effective order. Sentry is missing.

**Relevant files:**
- `src/middleware/mod.rs`
- `src/lib.rs`
- `src/bin/server.rs`
- `Cargo.toml`

**Prompt:**

> Rebuild the middleware stack. Because `Router::layer` runs **after routing**, some layers must wrap the whole `Router` in `build_handler` so they can observe and modify requests before routing.
>
> **Top-level services (wrap the whole `Router` in `src/lib.rs::build_handler`):**
> 1. Sentry (`NewSentryLayer` then `SentryHttpLayer` — see note below)
> 2. Custom path normalization (Task 2.2)
> 3. `CompressionLayer` (Fastest)
> 4. `RequestBodyTimeoutLayer` (30s)
> 5. `TimeoutLayer` (30s)
> 6. `CorsLayer` (only if origins configured)
> 7. `security_headers`
>
> **Per-route `Router::layer` stack (inside `apply_axum_middleware`):**
> 1. `require_user_agent`
> 2. `block_traffic`
> 3. `CatchPanicLayer`
> 4. `request_id`
> 5. `log_request` (Task 2.4)
> 6. `error_handler`
> 7. `metrics` update layer (feature-gated)
> 8. `session_middleware`
> 9. `ensure_token` (CSRF token)
> 10. `real_ip`
> 11. `authenticate`
> 12. `rate_limit`
> 13. `verify_origin` (CSRF origin check)
> 14. Handler
>
> The per-route order is from outermost (first) to innermost (last). In `apply_axum_middleware`, add `Router::layer` calls in the reverse of this order (innermost first).
>
> Implement Sentry behind a feature, enabled only when `SENTRY_DSN` is set. Use `sentry` and `sentry-tower`. Because of Axum's layer ordering, apply **both** `sentry_tower::NewSentryLayer::<Request<Body>>::new_from_top()` and `sentry_tower::SentryHttpLayer::new().enable_transaction()`; the Hub layer must be outside the HTTP layer.
>
> Update `src/lib.rs::build_handler` so it returns a generic `Service` or uses `ServiceBuilder::service(router)` rather than a raw `Router`. Update `src/bin/server.rs` and `src/tests/test_app.rs` accordingly.
>
> Update tests in `src/tests/middleware.rs` to verify the new ordering and that security headers appear on 404/500 responses.
>
> Keep `debug_requests` in Development, but place it before `log_request` so it does not interfere with Sentry.

**Acceptance criteria:**
- Middleware order matches the list, with top-level layers wrapping the `Router`.
- `real_ip` is available in `block_traffic`, `rate_limit`, and `authenticate`.
- Path normalization runs before routing.
- Panic and 404 responses include security headers.
- Sentry is optional and compiles behind a feature.

---

### 2.2 Implement full path normalization

**Why:** `NormalizePathLayer::trim_trailing_slash()` only trims trailing slashes. crates.io also collapses `//`, `/./`, and `/../`.

**Relevant files:**
- `src/middleware/normalize_path.rs` (new)
- `src/lib.rs`

**Prompt:**

> Implement a custom path normalization middleware and apply it as a top-level service around the `Router`, as configured in Task 2.1.
>
> 1. Create `src/middleware/normalize_path.rs` with a Tower `Layer` / `Service` or an `axum::middleware` function.
> 2. The middleware must, before the request reaches the `Router`:
>    - Collapse multiple consecutive slashes (`//` -> `/`).
>    - Remove `/./` segments.
>    - Resolve `/../` segments against the path prefix (reject paths that escape the root with 400).
>    - Trim trailing slash except for the root path `/`.
> 3. In `src/lib.rs::build_handler`, add the path normalization layer as the innermost top-level layer around the `Router` (just before the `Router` is handed to the outer top-level layers from Task 2.1).
> 4. Add unit tests for all cases.

**Acceptance criteria:**
- `/foo//bar` resolves to `/foo/bar`.
- `/foo/./bar` resolves to `/foo/bar`.
- `/foo/../bar` resolves to `/bar`.
- `/foo/../../etc/passwd` returns 400.
- `/foo/` resolves to `/foo`.

---

### 2.3 Pass real trusted proxies to `real_ip`

**Why:** `real_ip` ignores the parsed `TRUSTED_PROXIES` and uses a hardcoded localhost list.

**Relevant files:**
- `src/middleware/real_ip.rs`
- `src/middleware/mod.rs`

**Prompt:**

> Fix `real_ip` to use the configured trusted proxies.
>
> 1. Change `real_ip` from `from_fn` to `from_fn_with_state` and receive `AppState`.
> 2. Read `state.config.trusted_proxies` and use that instead of the hardcoded list.
> 3. Support `X-Forwarded-For` with multiple IPs (leftmost untrusted IP is the client).
> 4. Optionally support the `Forwarded` header as a fallback.
> 5. Update `src/middleware/mod.rs` to use `from_fn_with_state`.

**Acceptance criteria:**
- `TRUSTED_PROXIES=10.0.0.0/8` makes `real_ip` trust requests from `10.x.x.x`.
- Unmatched socket IPs fall back to socket address.
- Tests cover trusted, untrusted, and malformed `X-Forwarded-For`.

---

### 2.4 Rewrite request logging with hashed sensitive headers

**Why:** crates.io logs status, duration, real IP, request ID, user agent, and SHA-256 hashes of `Authorization`/`Cookie`. The current log only has method and URI.

**Relevant files:**
- `src/middleware/mod.rs`
- `src/middleware/real_ip.rs`
- `src/middleware/request_id.rs`

**Prompt:**

> Rewrite `log_request` into a proper structured access log.
>
> 1. Emit a single `tracing::info!` event (inside a `tracing::info_span!("http_request", ...)`) with fields:
>    - `http.method`
>    - `http.url`
>    - `http.status_code`
>    - `duration_ms`
>    - `http.request.id`
>    - `network.client.ip`
>    - `http.user_agent`
>    - `http.request.headers.hashed_authorization`
>    - `http.request.headers.hashed_cookie`
> 2. SHA-256 hash the raw bytes of `Authorization` and `Cookie`. If missing, use an empty string.
> 3. Read `RealIp` and `RequestId` from request extensions.
> 4. The message string should be `"{method} {uri} -> {status} ({duration:?})"`.
> 5. Do not log body or query parameters.

**Acceptance criteria:**
- A sample log line contains the fields above.
- `Authorization` and `Cookie` are never in clear text.
- Real IP and request ID are populated.
- Existing tests pass.

---

### 2.5 Protect and extend the metrics endpoint

**Why:** crates.io serves metrics behind an auth token and includes DB pool metrics. The repo exposes `/metrics` openly.

**Relevant files:**
- `src/metrics.rs`
- `src/router.rs`
- `src/config/server.rs`
- `src/tests/metrics.rs`

**Prompt:**

> Harden the metrics endpoint.
>
> 1. Add `METRICS_TOKEN` to `Server` (optional `SecretString`).
> 2. Move the metrics route to `/api/private/metrics`. If `METRICS_TOKEN` is set, require `Authorization: Bearer <token>`; otherwise keep it open for dev.
> 3. Add DB pool metrics to `InstanceMetrics`:
>    - `db_pool_connections_total`
>    - `db_pool_connections_idle`
>    - `db_pool_wait_time_seconds` (histogram)
>    - `db_pool_timeouts_total`
> 4. Add a `kind` path or query parameter. `kind=instance` returns Prometheus text; `kind=service` returns 501 for now.
> 5. Update `src/tests/metrics.rs` to request `/api/private/metrics` and to test the 401 case when `METRICS_TOKEN` is configured.

**Acceptance criteria:**
- `/metrics` is public only when no token is configured.
- With a token, missing/invalid token returns 401.
- DB pool metrics appear in the Prometheus output.
- Metrics tests are updated.

---

### 2.6 Add production tracing and Sentry

**Why:** crates.io uses JSON tracing in production and Sentry for errors. The repo has a plain fmt layer and no Sentry.

**Relevant files:**
- `src/bin/server.rs`
- new `src/tracing.rs`
- `src/config/server.rs`
- `Cargo.toml`

**Prompt:**

> Set up production-ready tracing and optional Sentry.
>
> 1. Create `src/tracing.rs` with an `init_tracing(env: Env)` function:
>    - `Development`/`Test`: pretty fmt subscriber with `EnvFilter`.
>    - `Production`: JSON fmt subscriber with `EnvFilter`.
> 2. Replace the subscriber setup in `src/bin/server.rs` with a call to `init_tracing`.
> 3. Add Sentry initialization behind a feature, enabled only when `SENTRY_DSN` is set. Use `sentry` + `sentry-tracing` to capture errors and `ERROR`-level `tracing` events.
> 4. Configure Sentry before the middleware stack so panics are captured.

**Acceptance criteria:**
- Dev logs are human-readable; production logs are JSON.
- Sentry is optional and does not panic without a DSN.
- `tracing::error!` events appear in Sentry when enabled.

---

## Phase 3: Auth, Errors, and HTMX Frontend

### 3.1 Harden session cookies

**Why:** crates.io cookies are `HttpOnly`, `Secure`, `SameSite=Strict`, 90-day max age. The repo uses `Lax`, no `Secure`, no `max-age`.

**Relevant files:**
- `src/middleware/session.rs`
- `src/config/server.rs`
- `src/tests/auth.rs`

**Prompt:**

> Harden session cookies and key management.
>
> 1. In `src/middleware/session.rs`, build the cookie with:
>    - `.http_only(true)`
>    - `.secure(true)` in `Production` (or when `SESSION_COOKIE_SECURE=true`; use `false` in dev by default)
>    - `.same_site(cookie::SameSite::Strict)`
>    - `.max_age(Duration::days(90))`
> 2. Use `cookie::Key::derive_from(session_key.expose_secret().as_bytes())` (task 1.2 already makes `SESSION_KEY` a `SecretString`).
> 3. For the first version, do **not** implement session-key rotation. If you want rotation later, add `PREVIOUS_SESSION_KEYS` support then; it is non-trivial with `SignedCookieJar`.
> 4. Update `src/tests/auth.rs` to verify the new flags.

**Acceptance criteria:**
- `Set-Cookie` includes `HttpOnly; Secure; SameSite=Strict; Max-Age=7776000` in production.
- In development, `Secure` is omitted by default.
- Existing tests pass.

---

### 3.2 Enforce account locks and token expiration

**Why:** `User` already has `account_lock_reason` and `account_lock_until`, but cookie and token auth do not check them.

**Relevant files:**
- `src/models/user.rs`
- `src/middleware/auth.rs`
- `src/controllers/auth.rs`
- `src/controllers/token.rs`

**Prompt:**

> Enforce account lock status during authentication.
>
> 1. In `authenticate`, after extracting `user_id` from session, load the `User` and reject with `forbidden` if `account_lock_until` is in the future.
> 2. In `validate_token`, after loading the token, load the user and reject if locked.
> 3. Add `User::is_locked(&self) -> bool` in `src/models/user.rs` and use it everywhere.
> 4. Update existing tests and add tests for locked users with valid sessions and valid tokens.

**Acceptance criteria:**
- Locked users cannot access cookie-authenticated routes.
- Locked users cannot access token-authenticated routes.
- Existing tests pass.

---

### 3.3 Create the base Askama template and wire CSP/CSRF

**Why:** HTMX needs a consistent layout with a CSP nonce on scripts, a CSRF meta, and `hx-headers` on the body.

**Relevant files:**
- `templates/base.html`
- `templates/index.html`
- `templates/examples/*.html`
- `src/router.rs`

**Prompt:**

> Create a base Askama template and update existing templates.
>
> 1. Create `templates/base.html`:
>    ```html
>    <!DOCTYPE html>
>    <html lang="en">
>    <head>
>      <meta charset="utf-8">
>      <meta name="viewport" content="width=device-width, initial-scale=1">
>      <meta name="csrf-token" content="{{ ctx.csrf_token }}">
>      <title>{% block title %}{% endblock %}</title>
>      <link nonce="{{ ctx.csp_nonce }}" rel="stylesheet" href="/static/css/style.css">
>      <script nonce="{{ ctx.csp_nonce }}" src="/static/vendor/htmx.min.js"></script>
>      <script nonce="{{ ctx.csp_nonce }}" defer src="/static/vendor/alpine.js"></script>
>    </head>
>    <body hx-headers='{"X-CSRF-Token": "{{ ctx.csrf_token }}"}'>
>      {% block content %}{% endblock %}
>    </body>
>    </html>
>    ```
> 2. Create `src/router.rs` `PageContext`:
>    ```rust
>    #[derive(Clone, Debug)]
>    pub struct PageContext {
>        pub csrf_token: String,
>        pub csp_nonce: String,
>    }
>    ```
>    Update `HtmlTemplate` so that every template it renders must contain a `ctx: PageContext` field. `HtmlTemplate` should extract `CspNonce` and `csrf_token` from request extensions and build `PageContext` before rendering.
> 3. Convert `templates/index.html` and `examples/*.html` to `{% extends "base.html" %}` and add `ctx: PageContext` to their template structs.
> 4. Ensure `examples/contact.html` includes `{{ ctx.csrf_token }}` in forms/headers.

**Acceptance criteria:**
- `base.html` exists and is used by all full-page templates.
- `PageContext` exists and is passed to every template through `HtmlTemplate`.
- All `<script>` and `<link>` tags have `nonce="{{ ctx.csp_nonce }}"`.
- HTMX sends `X-CSRF-Token` on every request.
- Pages still render and tests pass.

---

### 3.4 Add HTML error pages and better DB error mapping

**Why:** crates.io returns HTML for browser requests and JSON for API requests. The repo returns JSON only and maps every unknown error to 500.

**Relevant files:**
- `src/util/errors.rs`
- `src/router.rs`
- `templates/error.html`
- `src/middleware/error_handler.rs`

**Prompt:**

> Improve error handling to support HTML and map DB errors correctly. Do this **after** Task 3.3 so `error.html` can extend `base.html`.
>
> 1. Create `templates/error.html` extending `base.html` with `{{ status }}` and `{{ message }}` blocks/fields.
> 2. Add an `HtmlError` Askama template and a helper to render it, taking `ctx: PageContext`, `status`, and `message`.
> 3. In `AppError::response`, choose the format based on:
>    - `HX-Request: true` -> small HTML fragment or `HX-Reswap: none`.
>    - `Accept: text/html` -> `templates/error.html`.
>    - Otherwise -> JSON.
> 4. Map Toasty errors:
>    - Not-found / row-missing -> 404.
>    - Pool timeout / acquire error -> 503.
>    - Other DB errors -> 500 (and Sentry if enabled).
> 5. Keep `CatchPanicLayer` turning panics into 500.

**Acceptance criteria:**
- Browser requests to unknown routes get a styled HTML 404.
- `HX-Request` errors are returned as HTML fragments.
- API requests get JSON errors.
- DB not-found returns 404; pool timeouts return 503.

---

### 3.5 Detect `HX-Request` and render partials

**Why:** For HTMX, the app should return partial HTML fragments when `HX-Request: true`.

**Relevant files:**
- `src/router.rs`
- `templates/server_time.html`
- `templates/server_time_partial.html` (new)
- `templates/examples/counter_partial.html`

**Prompt:**

> Add HTMX partial rendering support.
>
> 1. Update `HtmlTemplate<T>` to optionally carry a partial template `P`:
>    ```rust
>    pub struct HtmlTemplate<T, P = ()> { ... }
>    ```
>    where `P` also implements `Template`. Add a `with_partial(self, partial: P) -> Self` method.
> 2. If the request has `HX-Request: true` and a partial is configured, render `P`; otherwise render the full template `T`.
> 3. Add `templates/server_time_partial.html` and use the existing `counter_partial.html`.
> 4. Update `server_time` and `counter` handlers to provide a partial when `HX-Request` is expected.
> 5. If `HX-Request` is `true` and the response is an error, set `HX-Reswap: none` and return a small `<div class="error">...</div>` fragment.
>
> If the generic two-template approach is too awkward for Askama, an acceptable fallback is to have the handler explicitly choose the full or partial template based on `HX-Request` and return it directly.

**Acceptance criteria:**
- `hx-get` to `/api/server-time` returns only the partial HTML.
- Full page refresh returns the full page.
- HTMX errors do not replace the whole body.

---

## Phase 4: Core Services

Optional add-ons (email, S3, replicas, `NOTIFY/LISTEN`, etc.) are documented in `docs/OPTIONAL_ADDONS.md`.

### 4.1 Make the rate limiter atomic

**Why:** The current `RateLimiter` reads the bucket, computes tokens, and then updates. This is a race condition even with SQLite. A single atomic SQL statement fixes it on both SQLite and PostgreSQL.

**Relevant files:**
- `src/rate_limiter.rs`
- `src/models/rate_limit_bucket.rs`
- `src/db.rs`

**Prompt:**

> Rewrite the rate limiter to be atomic.
>
> 1. Keep the `RateLimitBucket` model but implement the token take as a single raw SQL statement per backend:
>    - PostgreSQL: `INSERT ... ON CONFLICT (bucket_key) DO UPDATE SET tokens = GREATEST(0, EXCLUDED.tokens + ...), last_refill = ... RETURNING tokens`.
>    - SQLite: `INSERT OR REPLACE` or `UPDATE ... RETURNING` inside a transaction.
>    Use `toasty::sql::statement` or `toasty::sql::query` to run the raw SQL.
> 2. Compute `tokens_to_add` and `new_tokens` inside the SQL expression, not in Rust. The expression will differ slightly between PostgreSQL and SQLite (`EXTRACT(EPOCH FROM ...)` vs `strftime('%s', ...)`).
> 3. If returned `tokens < 0`, return `RateLimitError` with `Retry-After`.
> 4. Keep the existing per-action config and tests.
>
> For admin rate-limit overrides, see `docs/OPTIONAL_ADDONS.md`.

**Acceptance criteria:**
- Concurrent calls for the same bucket cannot overspend.
- `Retry-After` header is still returned.
- Existing tests pass or are updated.

---

### 4.2 Implement a background worker queue

**Why:** Most non-trivial web apps eventually need background jobs. The repo has no queue. A simple database-backed worker is a reusable base component.

**Relevant files:**
- new `src/worker/mod.rs`
- new `src/worker/runner.rs`
- new `src/worker/jobs.rs`
- `src/models/mod.rs`
- `src/app.rs`
- `src/bin/main.rs`

**Prompt:**

> Implement a database-backed background job system.
>
> 1. Add a `BackgroundJob` Toasty model:
>    ```rust
>    #[derive(Debug, Model)]
>    pub struct BackgroundJob {
>        #[key]
>        #[auto]
>        pub id: u64,
>        pub queue: String,
>        pub job_type: String,
>        pub data: String, // JSON
>        pub retries: i32,
>        pub priority: i16,
>        pub run_at: jiff::Timestamp,
>        pub created_at: jiff::Timestamp,
>    }
>    ```
>    `run_at` is the next time the job should be attempted.
> 2. Implement a `Job` trait and `Runner` registry in `src/worker/`:
>    ```rust
>    pub trait Job: DeserializeOwned + Send + Sync + 'static {
>        const NAME: &'static str;
>        async fn run(&self, app: &App) -> anyhow::Result<()>;
>    }
>    ```
> 3. Implement the worker loop in `src/worker/runner.rs`:
>    - Use a polling loop with a short sleep (e.g. 1–5 seconds).
>    - Claim the next available job with a row-level lock: `SELECT ... FOR UPDATE` in PostgreSQL or `BEGIN IMMEDIATE` / a single Toasty transaction in SQLite.
>    - Order by `priority DESC, run_at ASC, created_at ASC`.
>    - Delete on success; on failure, increment `retries` and set `run_at = now() + (2 ^ retries) seconds`.
>    - (Optional, PG only) Use `NOTIFY background_jobs` on insert and `LISTEN` in the worker to wake immediately. If you skip this, keep the polling loop.
> 4. Wire the `background-worker` subcommand to start the runner.
> 5. Provide an `App::enqueue_job(&self, job: impl Job)` helper.

**Acceptance criteria:**
- Jobs can be enqueued and processed by `cargo run -- background-worker`.
- Failed jobs are retried with exponential backoff.
- Jobs are idempotent (design the first example job, e.g. a cleanup or report job, to be idempotent).
- Worker compiles and starts on SQLite without `NOTIFY/LISTEN`.

---

Optional add-ons such as **email**, **S3/object storage**, **PostgreSQL test harness**, **read-only replicas**, and **rate-limit overrides** are documented in `docs/OPTIONAL_ADDONS.md`. Do not implement them unless the target app needs them.

---

## Phase 5: Testing and CI

### 5.1 Add snapshot tests and an optional PostgreSQL test harness

**Why:** Snapshot tests make refactors safe. `insta` is already a dev dependency. A PostgreSQL test harness is useful for apps that run on PostgreSQL in production, but it is optional for the SQLite-only base.

**Relevant files:**
- `src/tests/test_app.rs`
- new `src/tests/test_db.rs`
- `src/tests/auth.rs`
- `src/tests/error_responses.rs`
- `justfile`

**Prompt:**

> Improve the test harness and add snapshot coverage.
>
> 1. Add `insta` snapshot tests for:
>    - HTML home page
>    - HTML error page
>    - JSON API error responses
>    - Prometheus metrics output (when `metrics` feature is enabled)
> 2. Store snapshots under `src/tests/snapshots/`.
> 3. Add `just snapshot` and `just snapshot-accept` recipes.
>
> For a full PostgreSQL test harness, see `docs/OPTIONAL_ADDONS.md`.

**Acceptance criteria:**
- Snapshot tests exist and pass.
- SQLite test harness still works.

---

### 5.2 Fix environment-variable test isolation

**Why:** `cargo test --all-features` currently flakes on `config::server::tests::test_parse_blocked_traffic_empty` because not all env-mutating tests share the same lock.

**Relevant files:**
- `src/config/server.rs`

**Prompt:**

> Fix the env-var test isolation.
>
> 1. Ensure every test that sets or removes `BLOCKED_TRAFFIC` and related env vars acquires the shared `ENV_LOCK` and restores the original value on panic/unwind.
> 2. Alternatively, refactor `parse_blocked_traffic` to take the env value as an argument and provide a thin wrapper for production.
> 3. Verify `cargo test --all-features` passes 10 times in a row.

**Acceptance criteria:**
- `cargo test --all-features` passes consistently.
- No race conditions between env-mutating tests.

---

### 5.3 Harden CI and dependency management

**Why:** crates.io pins dependencies, runs `cargo-deny`, `cargo-machete`, `cargo audit`, and `zizmor`. The repo has `deny` and `machete` but not audit or zizmor, and `Cargo.toml` uses caret versions.

**Relevant files:**
- `.github/workflows/ci.yml`
- `justfile`
- `Cargo.toml`
- `Cargo.lock`

**Prompt:**

> Harden CI and dependency pinning.
>
> 1. Add a `cargo audit` step to `.github/workflows/ci.yml` and the `justfile`.
> 2. Add a `zizmor` step for workflow files.
> 3. Pin critical production dependencies in `Cargo.toml` with `=` to the exact current patch version (read the version from `Cargo.lock`). Start with: `axum`, `tokio`, `tower`, `tower-http`, `toasty`, `askama`, `cookie`, `oauth2`, `reqwest`. Leave dev/test dependencies with caret if desired.
> 4. Run `cargo update` and commit the resulting `Cargo.lock`.
> 5. Add `cargo insta test` to the CI test job (or use `cargo nextest run --all-features` with `INSTA_UPDATE=new` if you prefer).

**Acceptance criteria:**
- CI runs `cargo audit` and `zizmor`.
- Critical dependencies are pinned with `=`.
- `Cargo.lock` is up to date.
- Snapshot tests are part of CI.

---

## Skipped or optional features

Do **not** implement these in the base template unless the target app actually needs them. Most are documented as optional add-ons in `docs/OPTIONAL_ADDONS.md`:

- Email sending.
- S3/object storage.
- Read-only database replicas.
- PostgreSQL `NOTIFY/LISTEN` worker wake-up (polling is the base).
- PostgreSQL test harness.
- Rate-limit admin overrides.
- The `monitor` subcommand.
- CDN-specific Fastly/CloudFront invalidation logic.
- `cargo_compat` middleware or Cargo API endpoints.
- docs.rs integration.
- Git index management.
- GitHub OAuth team synchronization.

---

## Quick reference: verification commands

Run these after every task and in CI:

```bash
# Compile with all optional features
cargo check --all-features

# Lint with warnings as errors
cargo clippy --all-targets --all-features

# Run tests
cargo test --all-features

# Dependency / security checks (run periodically)
cargo deny check
cargo machete
cargo audit

# Snapshot tests
cargo insta test --all-features
cargo insta accept
```

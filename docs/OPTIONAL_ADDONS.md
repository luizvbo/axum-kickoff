# Optional Add-ons

This file contains GenAI prompts for features that are **not** part of the base template. Implement them only if the target application actually needs them.

These prompts assume the base template from `docs/CRATES_IO_IMPLEMENTATION_PLAN.md` is already in place.

---

## O.1 Add email support

**When to use:** Your app needs to send transactional or notification email.

**Relevant files:**
- new `src/email.rs`
- `src/worker/jobs.rs`
- `src/config/server.rs`
- `Cargo.toml`

**Prompt:**

> Add a configurable email abstraction.
>
> 1. Add `lettre` to `Cargo.toml`.
> 2. Create `src/email.rs` with an `Email` service and `EmailBackend` enum:
>    - `Smtp` (uses `lettre::SmtpTransport`)
>    - `File { path: PathBuf }` (writes `.eml` files for dev)
>    - `Memory` (stores in a vector for tests)
> 3. Support env vars: `EMAIL_BACKEND=file|smtp|memory`, `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `EMAIL_FROM`. All secrets are `SecretString`.
> 4. Define text and HTML Askama templates under `templates/emails/`.
> 5. Mask recipient addresses in logs (e.g. `u***@e***.com`) and hash the domain with SHA-256.
> 6. Send email only via a background job, never directly from a handler.

**Acceptance criteria:**
- `Email` service can be configured to file, memory, or SMTP.
- Full email addresses are never logged.
- Emails are queued and sent by the worker.
- Tests for all three backends.

---

## O.2 Add S3/object storage backend

**When to use:** Your app stores user files in S3, MinIO, R2, or a compatible object store.

**Relevant files:**
- `src/storage.rs`
- `src/config/server.rs`
- `Cargo.toml`

**Prompt:**

> Extend storage to support S3 and set cache headers.
>
> 1. Add `object_store` with the `aws` feature to `Cargo.toml`.
> 2. Extend `StorageConfig` and `StorageBackend` to support `LocalFileSystem` (existing), `S3` (via `object_store::aws::AmazonS3`), and `Memory` (via `object_store::memory::InMemory` for tests).
> 3. Support env vars: `STORAGE_BACKEND=local|s3|memory`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, `S3_BUCKET`, `S3_ENDPOINT` (for MinIO), `STORAGE_PATH`, `CDN_PREFIX`.
> 4. Add `Cache-Control` on upload/download based on object class:
>    - Immutable files -> `public, max-age=31536000, immutable`
>    - Other files -> `private, max-age=604800`
> 5. Add cache tags and background invalidation hooks as a no-op stub first.
> 6. Keep local as default for dev/tests.

**Acceptance criteria:**
- `STORAGE_BACKEND=memory` works in tests.
- `STORAGE_BACKEND=s3` with `S3_ENDPOINT` works with MinIO.
- Downloaded files include the correct `Cache-Control`.
- Existing local storage tests pass.

---

## O.3 Add a PostgreSQL test harness

**When to use:** Your production app runs on PostgreSQL and you want the test suite to exercise the same backend.

**Relevant files:**
- `src/tests/test_app.rs`
- new `src/tests/test_db.rs`
- `justfile`

**Prompt:**

> Add PostgreSQL support to the test harness.
>
> 1. Create `src/tests/test_db.rs` with a `TestDatabase` struct:
>    - If `TEST_DATABASE_URL` is set (PostgreSQL), create a unique DB from a template and drop it on `Drop`. Creating/dropping databases requires an admin connection (e.g. via `tokio-postgres` or `sqlx`) to the `postgres` maintenance DB; Toasty itself cannot create a database it is not already connected to.
>    - If not set, fall back to a temp SQLite file.
> 2. Update `TestApp` to use `TestDatabase`.
> 3. Add `just test-pg` and `just test-pg-accept` recipes if useful.

**Acceptance criteria:**
- `cargo test --all-features` runs with `TEST_DATABASE_URL` pointing at PostgreSQL.
- SQLite fallback still works when `TEST_DATABASE_URL` is not set.

---

## O.4 Add PostgreSQL `NOTIFY/LISTEN` worker wake-up

**When to use:** The worker runs on PostgreSQL and you want instant wake-up when new jobs are enqueued instead of polling.

**Relevant files:**
- `src/worker/runner.rs`
- `src/app.rs`

**Prompt:**

> Add instant wake-up for the background worker on PostgreSQL.
>
> 1. When a job is enqueued on PostgreSQL, run `NOTIFY background_jobs, '<job_id>'` via `toasty::sql::statement`.
> 2. In the worker loop, when connected to PostgreSQL, call `LISTEN background_jobs` on a dedicated connection and wait on a `tokio_postgres` notification. When a notification arrives, immediately check for a new job before resuming the polling sleep.
> 3. Keep the polling fallback for SQLite and for cases where the `LISTEN` connection drops.
> 4. Make this behavior conditional on the database backend; it must compile and run on SQLite without the `NOTIFY/LISTEN` code paths.

**Acceptance criteria:**
- Jobs are processed immediately on PostgreSQL without waiting for the next poll interval.
- SQLite still uses polling.
- Worker compiles and starts on SQLite.

---

## O.5 Add read-only database replicas

**When to use:** Your app runs on PostgreSQL and you want reads to go to a read replica, falling back to the primary.

**Relevant files:**
- `src/db.rs`
- `src/config/database.rs`
- `src/app.rs`

**Prompt:**

> Add optional read-only replica support.
>
> 1. Add `READ_ONLY_REPLICA_URL` and `READ_ONLY_REPLICA_SSLMODE` to `DatabaseConfig`.
> 2. Create a second `Database` for the replica in `App`.
> 3. Update `App::db_read()` to prefer the replica and fall back to the primary, emitting a `tracing::warn!` on fallback.
> 4. Set `default_transaction_read_only = on` for the replica by appending `options=-c%20default_transaction_read_only%3Don` to the replica URL.
> 5. Make this optional: if no `READ_ONLY_REPLICA_URL` is configured, `db_read()` returns the primary.

**Acceptance criteria:**
- `db_read()` returns the replica when configured.
- Fallback to the primary works and is logged.
- SQLite / single-database mode still works.

---

## O.6 Add rate-limit admin overrides

**When to use:** You need to grant extra rate-limit capacity to specific users or actions.

**Relevant files:**
- `src/rate_limiter.rs`
- `src/models/mod.rs`

**Prompt:**

> Add admin rate-limit overrides.
>
> 1. Add a `RateLimitOverride` Toasty model with `user_id`, `action`, `tokens`, `expires_at`.
> 2. In `RateLimiter::check_rate_limit`, check for an active override for `(bucket_id, action)` before computing the bucket state. If found, use the override `tokens` instead.
> 3. Expire overrides where `expires_at < now()`.
> 4. Keep the atomic SQL-based token take from the base template.

**Acceptance criteria:**
- A user with an active override gets the override token budget.
- Expired overrides are ignored.
- Existing rate-limit tests still pass.

---

## O.7 Add a `monitor` subcommand

**When to use:** You want a command that reports queue health, DB connectivity, and other ops metrics and exits, suitable for a Kubernetes liveness/readiness probe.

**Relevant files:**
- `src/bin/main.rs`
- `src/app.rs`

**Prompt:**

> Add a `monitor` subcommand.
>
> 1. Register a `monitor` subcommand in the clap app.
> 2. Implement `run_monitor()` that opens a short-lived `App`, checks that the database is reachable, reports the number of pending background jobs per queue, and exits with code 0 if healthy or non-zero if not.
> 3. Keep the output simple (plain text or JSON) and suitable for a probe.

**Acceptance criteria:**
- `cargo run -- monitor` exits 0 when the app is healthy.
- `cargo run -- monitor` exits non-zero when the DB is unreachable.

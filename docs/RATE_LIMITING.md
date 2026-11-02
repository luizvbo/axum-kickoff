# Rate Limiting

This document describes the rate limiting system in {{project-name}}.

## Overview

Rate limiting protects your application from abuse by throttling requests
based on action types. {{project-name}} ships exactly **one** backend: a
database-backed token bucket (`src/rate_limiter.rs`). There is no in-memory
or Redis backend — bucket state lives in the `rate_limit_buckets` table, so
it persists across restarts and is shared by every instance connected to the
same database.

## Algorithm

The rate limiter uses the **token bucket algorithm**:

- **Burst**: Maximum number of tokens the bucket can hold
- **Rate**: One token refills every `rate` seconds
- Each request consumes one token; if none are available the request is
  rejected with `429 Too Many Requests`

The token take is a single atomic SQL upsert (`INSERT ... ON CONFLICT DO
UPDATE ... RETURNING tokens`), so concurrent requests for the same bucket
cannot overspend. `last_refill` only advances by whole-refilled-token
intervals, preserving fractional refill progress.

## Bucket Keys

Each bucket is identified by `{action}:{bucket_id}` where `bucket_id` is:

- the authenticated `user_id`, when the session carries one, or
- the client's real IP address (see `real_ip` middleware)

## Action Types

`LimitedAction` (in `src/rate_limiter.rs`) defines nine actions. Which one
applies to a request is decided by `determine_limited_action` in
`src/middleware/mod.rs` — a per-path mapping, applied by the `rate_limit`
route layer:

| Action | Default Rate | Default Burst | Routed at |
|--------|--------------|---------------|-----------|
| `ApiRequest` | 1 req/sec | 10 | catch-all for rate-limited routes |
| `TokenCreation` | 10 sec/req | 3 | `POST /api/v1/tokens` |
| `OAuthAuthorize` | 5 sec/req | 5 | `GET /api/v1/auth/{provider}/authorize` |
| `OAuthCallback` | 5 sec/req | 5 | `GET /api/v1/auth/{provider}/callback` |
| `FormSubmission` | 30 sec/req | 10 | `POST /examples/contact`, logout |
| `PublicApiRead` | 1 req/sec | 60 | anonymous `GET /api/v1/posts*` |
| `LoginAttempt` | 5 sec/req | 5 | defined, not wired to a route yet |
| `PasswordReset` | 60 sec/req | 3 | defined, not wired to a route yet |
| `FileUpload` | 10 sec/req | 5 | defined, not wired to a route yet |

## Configuration

Each action is configurable via environment variables:

```bash
RATE_LIMITER_API_REQUEST_RATE_SECONDS=1
RATE_LIMITER_API_REQUEST_BURST=10

RATE_LIMITER_TOKEN_CREATION_RATE_SECONDS=10
RATE_LIMITER_TOKEN_CREATION_BURST=3

RATE_LIMITER_FORM_SUBMISSION_RATE_SECONDS=30
RATE_LIMITER_FORM_SUBMISSION_BURST=10

RATE_LIMITER_PUBLIC_API_READ_RATE_SECONDS=1
RATE_LIMITER_PUBLIC_API_READ_BURST=60

# ...same pattern for OAUTH_AUTHORIZE, OAUTH_CALLBACK,
# LOGIN_ATTEMPT, PASSWORD_RESET, FILE_UPLOAD
```

Note: the env vars exist for all nine actions, but configuring an action
that no route maps to (`LoginAttempt`, `PasswordReset`, `FileUpload`) has
no effect until you wire it up in `determine_limited_action`.

## Schema

```sql
CREATE TABLE rate_limit_buckets (
    bucket_key TEXT PRIMARY KEY,  -- "{action}:{bucket_id}"
    action     TEXT NOT NULL,
    bucket_id  TEXT NOT NULL,     -- user ID or IP address
    tokens     INTEGER NOT NULL,
    last_refill TIMESTAMP NOT NULL
);
```

There is no `rate_limit_overrides` table and no per-user override
mechanism.

## Applying Rate Limiting to a Route

Use the `rate_limit` middleware as a `route_layer` — the same way
`POST /examples/contact` does it in `src/router.rs`:

```rust
.route(
    "/posts",
    post(create_post).route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        rate_limit,
    )),
)
```

The middleware picks the action from the request method+path via
`determine_limited_action` — add a match arm there if your route needs a
non-`ApiRequest` budget. For direct checks (e.g. inside a handler):

```rust
app.rate_limiter
    .check_rate_limit(&user_id.to_string(), LimitedAction::ApiRequest)
    .await?;
// or by IP: .check_by_ip(ip, LimitedAction::ApiRequest)
```

`clear_all()` and `clear_key(key, action)` exist for tests.

## Rate Limit Response

When limited, the middleware returns `429 Too Many Requests` with a
`Retry-After` header (the action's refill interval) and a per-action
message such as "Too many requests. Please slow down."

## Scaling Beyond a Single Database

The database backend already shares state across instances. If you outgrow
it (very high request volume, or you don't want rate-limit writes on the
primary DB), [rate-limiting-redis-upgrade.md](rate-limiting-redis-upgrade.md)
walks through adding a Redis backend — it is a design document, not shipped
code.

## Troubleshooting

### Rate Limiting Not Working

- Verify the `rate_limit` `route_layer` is applied to the route — only
  layered routes are throttled; middleware does not run for unrouted paths.
- Check that the action env vars use the exact `RATE_LIMITER_<ACTION>_*` names.
- Confirm the client isn't sharing a bucket (all traffic behind one proxy
  IP — check `TRUSTED_PROXIES` / `real_ip` configuration).

### Too Many Rate Limit Errors

- Increase burst capacity or shorten the rate interval via env vars
- Verify `TRUSTED_PROXIES` is set so `X-Forwarded-For` gives per-client
  buckets instead of one shared proxy bucket
- Check `rate_limit_buckets` rows for the affected key

### Stuck Buckets

`last_refill` semantics preserve fractional refill; if a bucket looks
stuck, inspect its row directly — the token count can go to `-1` (the
limiter stores the failed spend as a negative token balance that refills
back to zero).

## See Also

- [Configuration Documentation](CONFIGURATION.md#rate-limiting-configuration)
- [Middleware Documentation](MIDDLEWARE.md)
- [Redis upgrade design](rate-limiting-redis-upgrade.md)
- [Architecture Documentation](ARCHITECTURE.md#rate-limiting)

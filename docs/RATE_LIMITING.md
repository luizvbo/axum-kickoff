# Rate Limiting

This document describes the rate limiting system in axum-kickoff, including in-memory, database-backed, and Redis implementations.

## Overview

Rate limiting protects your application from abuse by throttling requests based on action types. axum-kickoff supports multiple backends:

- **In-Memory**: Default for single-instance deployments
- **Database-Backed**: For distributed systems (SQLite/PostgreSQL)
- **Redis**: For high-throughput distributed systems

## Algorithm

The rate limiter uses the **token bucket algorithm**:

- **Burst**: Maximum number of requests allowed instantly
- **Rate**: Rate at which tokens are refilled (requests per second)
- **Tokens**: Current available tokens
- **Refill**: Tokens are refilled continuously at the configured rate

### How It Works

1. Each request consumes one token
2. If tokens are available, request proceeds
3. If no tokens available, request is rate-limited
4. Tokens are refilled continuously at the configured rate
5. Tokens cannot exceed the burst capacity

## Action Types

Rate limiting is configured per action type:

| Action | Description | Default Rate | Default Burst |
|--------|-------------|--------------|---------------|
| `ApiRequest` | General API requests | 1 request/sec | 10 requests |
| `LoginAttempt` | Login attempts | 1 request/5sec | 5 requests |
| `PasswordReset` | Password reset requests | 1 request/60sec | 3 requests |
| `FileUpload` | File upload requests | 1 request/10sec | 5 requests |
| `FormSubmission` | Form submissions | 1 request/30sec | 10 requests |

## In-Memory Rate Limiting

### Overview

In-memory rate limiting uses `Arc<RwLock<HashMap>>` for storage. It's simple and fast but not distributed.

### Configuration

```bash
# Configure per-action limits
RATE_LIMITER_API_REQUEST_RATE_SECONDS=1
RATE_LIMITER_API_REQUEST_BURST=10

RATE_LIMITER_LOGIN_ATTEMPT_RATE_SECONDS=5
RATE_LIMITER_LOGIN_ATTEMPT_BURST=5

RATE_LIMITER_PASSWORD_RESET_RATE_SECONDS=60
RATE_LIMITER_PASSWORD_RESET_BURST=3

RATE_LIMITER_FILE_UPLOAD_RATE_SECONDS=10
RATE_LIMITER_FILE_UPLOAD_BURST=5

RATE_LIMITER_FORM_SUBMISSION_RATE_SECONDS=30
RATE_LIMITER_FORM_SUBMISSION_BURST=10
```

### Pros

- **Simple**: No external dependencies
- **Fast**: Sub-millisecond latency
- **Zero Setup**: Works out of the box

### Cons

- **Not Distributed**: Each instance has separate state
- **No Persistence**: Data lost on restart
- **No Overrides**: No per-user rate limit overrides

### When to Use

- Single-instance deployments
- Development environments
- Low to moderate traffic
- When simplicity is preferred

## Database-Backed Rate Limiting

### Overview

Database-backed rate limiting stores rate limit data in the database, enabling distributed deployments.

### Configuration

```bash
# Use PostgreSQL for distributed rate limiting
DATABASE_URL=postgresql://user:password@host:5432/dbname
```

### Implementation

Rate limit data is stored in database tables:

```sql
CREATE TABLE rate_limit_buckets (
    id TEXT PRIMARY KEY,
    tokens INTEGER NOT NULL,
    last_refill TIMESTAMP NOT NULL
);

CREATE TABLE rate_limit_overrides (
    user_id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    burst INTEGER NOT NULL,
    expires_at TIMESTAMP
);
```

### Pros

- **Distributed**: Shared state across instances
- **Persistent**: Data survives restarts
- **Overrides**: Per-user rate limit overrides
- **Time-Limited**: Temporary rate limit increases

### Cons

- **Database Dependency**: Requires database access
- **Latency**: Higher latency than in-memory
- **Complexity**: More complex setup

### When to Use

- Multi-instance deployments
- Production environments
- When per-user overrides are needed
- When persistence is required

## Redis Rate Limiting

### Overview

Redis rate limiting provides high-performance distributed rate limiting with sub-millisecond latency.

### When to Upgrade to Redis

Consider upgrading to Redis when:
- You need to run multiple application instances behind a load balancer
- You require sub-millisecond rate limit checks
- You need to handle high request volumes (10,000+ requests per second)
- You want distributed rate limiting across multiple data centers

### Implementation Steps

#### 1. Add Redis Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
```

#### 2. Create Redis Rate Limiter

Create `src/rate_limiter_redis.rs`:

```rust
use redis::{AsyncCommands, Client, ConnectionManager};
use std::time::Duration;
use std::collections::HashMap;

pub struct RedisRateLimiter {
    client: ConnectionManager,
    config: HashMap<String, RateLimiterConfig>,
}

impl RedisRateLimiter {
    pub async fn new(redis_url: &str, config: HashMap<String, RateLimiterConfig>) -> Result<Self, redis::RedisError> {
        let client = Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { client: conn, config })
    }

    pub async fn check_rate_limit(&self, key: &str, action: &str) -> Result<(), RateLimitError> {
        let config = self.config.get(action).unwrap();
        let redis_key = format!("rate_limit:{}:{}", action, key);

        // Use Redis Lua script for atomic token bucket operations
        let script = r#"
            local key = KEYS[1]
            local burst = tonumber(ARGV[1])
            local refill_rate = tonumber(ARGV[2])
            local now = tonumber(ARGV[3])

            local bucket = redis.call('HMGET', key, 'tokens', 'last_refill')
            local tokens = tonumber(bucket[1])
            local last_refill = tonumber(bucket[2])

            if not tokens then
                tokens = burst - 1
                redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
                redis.call('EXPIRE', key, 3600)
                return 1
            end

            local elapsed = now - last_refill
            local tokens_to_add = math.floor(elapsed / refill_rate)

            if tokens_to_add > 0 then
                tokens = math.min(tokens + tokens_to_add, burst)
            end

            if tokens >= 1 then
                tokens = tokens - 1
                redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
                redis.call('EXPIRE', key, 3600)
                return 1
            else
                return 0
            end
        "#;

        let mut conn = self.client.clone();
        let result: i32 = redis::Script::new(script)
            .key(&redis_key)
            .arg(config.burst)
            .arg(config.rate.as_secs())
            .arg(chrono::Utc::now().timestamp())
            .invoke_async(&mut conn)
            .await?;

        if result == 1 {
            Ok(())
        } else {
            Err(RateLimitError::new(action, config.rate))
        }
    }
}
```

#### 3. Update Configuration

Add Redis configuration to `src/config/server.rs`:

```rust
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub backend: RateLimitBackend,
    pub redis_url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RateLimitBackend {
    Sqlite,
    Redis,
}
```

#### 4. Update App Initialization

Modify `src/app.rs` to support both backends:

```rust
impl App {
    pub async fn new(config: config::Server, database: Database) -> Result<Self, anyhow::Error> {
        let rate_limiter = match config.rate_limit.backend {
            RateLimitBackend::Sqlite => {
                RateLimiter::new(config.rate_limit.config, database.clone())
            }
            RateLimitBackend::Redis => {
                let redis_url = config.rate_limit.redis_url.expect("Redis URL required");
                RedisRateLimiter::new(&redis_url, config.rate_limit.config).await?
            }
        };

        // ... rest of initialization
    }
}
```

### Migration Strategy

#### Phase 1: Dual-Write

- Continue using SQLite for rate limiting
- Add Redis as a secondary backend
- Write rate limit data to both systems
- Monitor for consistency

#### Phase 2: Read-Through

- Read from Redis first, fall back to SQLite
- Continue writing to both systems
- Gradually increase Redis traffic

#### Phase 3: Redis-Only

- Switch to Redis-only operations
- Keep SQLite as backup for 1-2 weeks
- Remove SQLite rate limiting after validation

### Performance Comparison

| Metric | SQLite | Redis |
|--------|--------|-------|
| Latency | 1-5ms | <1ms |
| Throughput | ~5K req/s | ~50K req/s |
| Scalability | Single instance | Distributed |
| Persistence | Disk | Memory (with persistence) |
| Cost | Free | Requires Redis instance |

### Redis Configuration Recommendations

#### Development

```bash
# Use local Redis instance
redis-server
```

#### Production

```bash
# Use managed Redis service (AWS ElastiCache, Redis Cloud, etc.)
# Or deploy with persistence:
redis-server --save 60 1000 --appendonly yes
```

#### Key Settings

- `maxmemory`: Set to 80% of available RAM
- `maxmemory-policy`: `allkeys-lru` for automatic eviction
- `save`: Enable RDB snapshots for persistence
- `appendonly`: Enable AOF for durability

### Monitoring

Monitor these Redis metrics:

- Memory usage (`INFO memory`)
- Connection count (`INFO clients`)
- Command statistics (`INFO commandstats`)
- Latency (`LATENCY LATEST`)

### Rollback Plan

If Redis causes issues:

1. Switch configuration back to SQLite backend
2. No data loss - SQLite remains as source of truth
3. Graceful degradation without downtime

### Cost Considerations

- **Self-hosted Redis**: Free if using existing infrastructure
- **Managed Redis**: $15-100/month depending on tier
- **Redis Cloud**: Free tier available for development
- **AWS ElastiCache**: $20-200/month depending on instance size

### Security

- Enable Redis authentication (`requirepass`)
- Use TLS for network connections
- Restrict network access with firewall rules
- Use VPC/private network for production

## Usage

### In Controllers

Apply rate limiting to endpoints:

```rust
use crate::rate_limiter::LimitedAction;
use crate::util::errors::AppResult;

pub async fn create_post(
    State(app): State<AppState>,
    // ... other parameters
) -> AppResult<Json<Post>> {
    // Check rate limit
    app.rate_limiter
        .check("user_123", LimitedAction::ApiRequest)
        .await?;

    // Process request
    // ...
}
```

### Rate Limit Response

When rate limited, the application returns:

```json
{
  "error": "Rate limit exceeded",
  "retry_after": 5
}
```

With HTTP status `429 Too Many Requests`.

## Per-User Overrides

### Configuration

Override rate limits for specific users:

```sql
INSERT INTO rate_limit_overrides (user_id, action, burst, expires_at)
VALUES ('user_123', 'ApiRequest', 100, '2024-12-31 23:59:59');
```

### Use Cases

- Trusted partners
- Power users
- Premium accounts
- Temporary increases for events

## Best Practices

### DO

- Set appropriate rate limits for your use case
- Monitor rate limit violations
- Log rate limit events for security
- Use distributed rate limiting for multi-instance deployments
- Set up alerts for unusual rate limit patterns
- Document rate limit policies to users

### DON'T

- Don't set rate limits too low (affects legitimate users)
- Don't set rate limits too high (reduces protection)
- Don't ignore rate limit violations
- Don't use in-memory rate limiting for distributed systems
- Don't forget to test rate limiting

## Troubleshooting

### Rate Limiting Not Working

- Verify rate limiter is initialized
- Check configuration values
- Verify action type is correct
- Check logs for errors

### Too Many Rate Limit Errors

- Increase burst capacity
- Increase refill rate
- Add per-user overrides
- Check for legitimate traffic patterns

### Redis Connection Issues

- Verify Redis is running
- Check connection string
- Verify network connectivity
- Check Redis authentication

### Performance Issues

- Monitor database/Redis performance
- Check for slow queries
- Consider caching rate limit checks
- Profile the rate limiter

## See Also

- [Configuration Documentation](CONFIGURATION.md#rate-limiting-configuration)
- [Middleware Documentation](MIDDLEWARE.md)
- [Architecture Documentation](ARCHITECTURE.md#rate-limiting)

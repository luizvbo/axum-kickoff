# Upgrading Rate Limiting to Redis

## Overview

The current rate limiting implementation uses SQLite for persistence, which works well for single-instance deployments. For distributed systems with multiple application instances, Redis is recommended for better performance and consistency.

## When to Upgrade to Redis

Consider upgrading to Redis when:
- You need to run multiple application instances behind a load balancer
- You require sub-millisecond rate limit checks
- You need to handle high request volumes (10,000+ requests per second)
- You want distributed rate limiting across multiple data centers

## Implementation Steps

### 1. Add Redis Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
```

### 2. Create Redis Rate Limiter

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

### 3. Update Configuration

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

### 4. Update App Initialization

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

### 5. Migration Strategy

**Phase 1: Dual-Write**
- Continue using SQLite for rate limiting
- Add Redis as a secondary backend
- Write rate limit data to both systems
- Monitor for consistency

**Phase 2: Read-Through**
- Read from Redis first, fall back to SQLite
- Continue writing to both systems
- Gradually increase Redis traffic

**Phase 3: Redis-Only**
- Switch to Redis-only operations
- Keep SQLite as backup for 1-2 weeks
- Remove SQLite rate limiting after validation

## Performance Comparison

| Metric | SQLite | Redis |
|--------|--------|-------|
| Latency | 1-5ms | <1ms |
| Throughput | ~5K req/s | ~50K req/s |
| Scalability | Single instance | Distributed |
| Persistence | Disk | Memory (with persistence) |
| Cost | Free | Requires Redis instance |

## Redis Configuration Recommendations

### Development
```bash
# Use local Redis instance
redis-server
```

### Production
```bash
# Use managed Redis service (AWS ElastiCache, Redis Cloud, etc.)
# Or deploy with persistence:
redis-server --save 60 1000 --appendonly yes
```

### Key Settings
- `maxmemory`: Set to 80% of available RAM
- `maxmemory-policy`: `allkeys-lru` for automatic eviction
- `save`: Enable RDB snapshots for persistence
- `appendonly`: Enable AOF for durability

## Monitoring

Monitor these Redis metrics:
- Memory usage (`INFO memory`)
- Connection count (`INFO clients`)
- Command statistics (`INFO commandstats`)
- Latency (`LATENCY LATEST`)

## Rollback Plan

If Redis causes issues:
1. Switch configuration back to SQLite backend
2. No data loss - SQLite remains as source of truth
3. Graceful degradation without downtime

## Cost Considerations

- **Self-hosted Redis**: Free if using existing infrastructure
- **Managed Redis**: $15-100/month depending on tier
- **Redis Cloud**: Free tier available for development
- **AWS ElastiCache**: $20-200/month depending on instance size

## Security

- Enable Redis authentication (`requirepass`)
- Use TLS for network connections
- Restrict network access with firewall rules
- Use VPC/private network for production

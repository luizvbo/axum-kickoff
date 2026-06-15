# Quickwit Integration

## Overview

Quickwit is a cloud-native search and analytics engine designed for log analytics and observability. It provides powerful search capabilities, efficient storage, and can ingest logs from various sources.

## Architecture

The recommended architecture for using Quickwit with axum-kickoff:

```
Application (axum-kickoff) → JSON Logs → File/Stdout → Vector/Fluentbit → Quickwit
```

This approach has several benefits:
- **No application code changes**: Your application simply outputs logs
- **Decoupled**: Log collection is handled by external infrastructure
- **Flexible**: Easy to switch log collectors or add multiple destinations
- **Production-ready**: Battle-tested log collectors handle edge cases

## Setup

### 1. Configure JSON Logging

Update your `src/bin/server.rs` to output JSON logs:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt};

fn main() -> anyhow::Result<()> {
    // Initialize logging with JSON format for Quickwit
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(
            fmt::layer()
                .json() // Output JSON format
                .with_current_span(true)
                .with_span_list(true)
        )
        .init();

    // ... rest of your application
}
```

### 2. Install Quickwit

Follow the [Quickwit installation guide](https://quickwit.io/docs/get-started/installation):

```bash
# Using cargo
cargo install quickwit

# Or download the binary
wget https://github.com/quickwit-oss/quickwit/releases/download/v0.8.0/quickwit-v0.8.0-x86_64-unknown-linux-gnu.tar.gz
tar -xzf quickwit-v0.8.0-x86_64-unknown-linux-gnu.tar.gz
```

### 3. Create a Quickwit Index

```bash
quickwit index create \
  --index-config quickwit-index-config.yaml
```

Example `quickwit-index-config.yaml`:

```yaml
version: 0.8
index_id: axum-kickoff-logs
doc_mapping:
  field_mappings:
    - name: timestamp
      type: datetime
      input_formats:
        - iso8601
      output_format: iso8601
      fast: true
    - name: level
      type: text
      tokenizer: raw
    - name: message
      type: text
      tokenizer: default
    - name: target
      type: text
      tokenizer: raw
    - name: span
      type: text
      tokenizer: raw
  timestamp_field: timestamp
indexing_settings:
  timestamp_field: timestamp
```

### 4. Configure Log Collector (Vector)

Install and configure Vector to collect logs and send them to Quickwit:

```toml
# vector.toml
[sources.file]
type = "file"
include = ["/var/log/axum-kickoff/*.log"]
read_from = "beginning"

[transforms.parse_json]
type = "remap"
inputs = ["file"]
source = """
. = parse_json!(.message)
.timestamp = parse_timestamp!(.timestamp, "%Y-%m-%dT%H:%M:%S%.fZ")
"""

[sinks.quickwit]
type = "http"
inputs = ["parse_json"]
uri = "http://localhost:7280/api/v1/indexes/axum-kickoff-logs/ingest"
encoding.json = true
compression = "gzip"
batch.max_events = 100
batch.timeout_secs = 1
```

### 5. Start Quickwit

```bash
# Start Quickwit server
quickwit run

# Ingest logs from file
quickwit index ingest \
  --index axum-kickoff-logs \
  --input-file /var/log/axum-kickoff/app.log
```

### 6. Query Logs

```bash
# Search for errors
quickwit index search \
  --index axum-kickoff-logs \
  --query "level:ERROR"

# Search for specific endpoint
quickwit index search \
  --index axum-kickoff-logs \
  --query "message:\"POST /api/users\""

# Aggregate by level
quickwit index search \
  --index axum-kickoff-logs \
  --query "*" \
  --aggs "level:terms(level)"
```

## Environment Variables

Configure logging behavior via environment variables:

```bash
# Set log level
export RUST_LOG=info

# Enable JSON logging (default in production)
export LOG_FORMAT=json

# Log file path (if writing to file)
export LOG_FILE=/var/log/axum-kickoff/app.log
```

## OpenTelemetry Integration

Quickwit also supports OpenTelemetry traces. To enable:

```rust
// Add to Cargo.toml
opentelemetry = { version = "0.27", features = ["trace"] }
opentelemetry-jaeger = { version = "0.27", features = ["rt-tokio"] }
tracing-opentelemetry = "0.27"

// In src/bin/server.rs
use opentelemetry::trace::TracerProvider;
use opentelemetry_jaeger::new_pipeline;
use tracing_opentelemetry::OpenTelemetryLayer;

fn main() -> anyhow::Result<()> {
    // Initialize OpenTelemetry
    let tracer = new_pipeline()
        .with_service_name("axum-kickoff")
        .install_simple()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(OpenTelemetryLayer::new(tracer))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // ... rest of application
}
```

## Benefits of Quickwit

- **Fast search**: Sub-second search across billions of log events
- **Cost-effective**: Efficient storage with columnar compression
- **Cloud-native**: Designed for modern observability stacks
- **Flexible**: Supports log aggregation, metrics, and traces
- **Open source**: No vendor lock-in

## Monitoring and Alerts

Set up monitoring on your Quickwit instance:

```bash
# Check index health
quickwit index describe --index axum-kickoff-logs

# Monitor ingestion rate
quickwit index search --index axum-kickoff-logs --query "*" --aggs "timestamp:histogram(timestamp,interval=1h)"
```

## Production Considerations

1. **Log Rotation**: Configure logrotate to manage log file sizes
2. **Retention**: Set up Quickwit index retention policies
3. **High Availability**: Deploy Quickwit in a distributed setup
4. **Security**: Enable authentication and TLS for Quickwit API
5. **Monitoring**: Monitor Quickwit health and ingestion lag

## Troubleshooting

### Logs not appearing in Quickwit
- Check Vector is running: `vector validate vector.toml`
- Verify Quickwit is accessible: `curl http://localhost:7280/health`
- Check log file permissions

### Search performance issues
- Optimize index configuration
- Add appropriate field mappings
- Consider partitioning by time

### High memory usage
- Adjust Vector buffer sizes
- Configure Quickwit memory limits
- Implement log sampling for high-volume scenarios

## References

- [Quickwit Documentation](https://quickwit.io/docs)
- [Quickwit GitHub](https://github.com/quickwit-oss/quickwit)
- [Vector Documentation](https://vector.dev/docs)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)

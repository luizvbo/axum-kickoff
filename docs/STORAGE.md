# Storage

This document describes the storage abstraction layer in axum-kickoff, which provides a unified interface for file operations across different backends.

## Overview

The storage layer provides a pluggable abstraction for file operations, supporting:

- **Local Filesystem**: Default for development
- **S3 Compatible**: AWS S3, MinIO, DigitalOcean Spaces (planned)
- **In-Memory**: For testing (planned)

The abstraction allows easy switching between backends without changing application code.

## Architecture

### Storage Abstraction

The storage layer is implemented in `src/storage.rs`:

```rust
pub struct Storage {
    cdn_prefix: Option<String>,
    base_path: PathBuf,
}
```

### Configuration

Storage is configured via `StorageConfig`:

```rust
pub struct StorageConfig {
    backend: StorageBackend,
    pub cdn_prefix: Option<String>,
}
```

## Local Filesystem Storage

### Configuration

Configure local filesystem storage via environment variables:

```bash
STORAGE_PATH=./uploads
CDN_PREFIX=cdn.example.com  # Optional
```

Or programmatically:

```rust
use axum_kickoff::storage::{Storage, StorageConfig};

let config = StorageConfig::local_filesystem("/tmp/uploads");
let storage = Storage::from_config(&config);
```

### With CDN Prefix

For serving files through a CDN:

```rust
let config = StorageConfig::local_filesystem_with_cdn(
    "/tmp/uploads",
    "cdn.example.com".to_string()
);
let storage = Storage::from_config(&config);
```

### Operations

#### Upload

```rust
use bytes::Bytes;

let bytes = Bytes::from_static(b"hello world");
storage.upload("test.txt", bytes).await?;
```

#### Upload with Content Type

```rust
storage.upload_with_content_type(
    "test.json",
    bytes,
    "application/json"
).await?;
```

Note: Local filesystem doesn't store content-type metadata. This is a no-op for local storage but kept for API compatibility with S3.

#### Download

```rust
let data = storage.download("test.txt").await?;
```

#### Delete

```rust
storage.delete("test.txt").await?;
```

#### Delete All with Prefix

```rust
let deleted = storage.delete_all_with_prefix("uploads/").await?;
// Returns list of deleted file paths
```

#### Check Existence

```rust
let exists = storage.exists("test.txt").await?;
```

#### List Files

```rust
// List all files
let all_files = storage.list(None).await?;

// List files with prefix
let uploads = storage.list(Some("uploads/")).await?;
```

#### Public URL

```rust
let url = storage.public_url("test.txt");
// Returns: "/test.txt" or "https://cdn.example.com/test.txt"
```

## S3 Storage (Planned)

### Configuration

```bash
STORAGE_BACKEND=s3
STORAGE_S3_BUCKET=your-bucket-name
STORAGE_S3_REGION=us-east-1
STORAGE_S3_ACCESS_KEY=your-access-key
STORAGE_S3_SECRET_KEY=your-secret-key
STORAGE_S3_ENDPOINT=https://s3.amazonaws.com  # Optional
CDN_PREFIX=cdn.example.com  # Optional
```

### S3-Compatible Services

The S3 backend will support:

- **AWS S3**: Amazon Simple Storage Service
- **MinIO**: Self-hosted S3-compatible storage
- **DigitalOcean Spaces**: S3-compatible object storage
- **Wasabi**: S3-compatible cloud storage
- **Other S3-compatible services**

### Operations

S3 storage will support the same operations as local filesystem:

- Upload with content-type metadata
- Download
- Delete
- List with prefix
- Check existence
- Public URL generation with CDN support

## In-Memory Storage (Planned)

### Use Case

In-memory storage is useful for:

- Testing
- Temporary file operations
- Caching

### Configuration

```bash
STORAGE_BACKEND=memory
```

### Limitations

- Data is lost on restart
- Not suitable for production
- Limited by available memory

## CDN Integration

### CDN Prefix

Configure a CDN prefix to generate CDN URLs:

```bash
CDN_PREFIX=cdn.example.com
```

### URL Generation

The storage layer automatically generates CDN URLs:

```rust
let url = storage.public_url("uploads/image.jpg");
// Returns: "https://cdn.example.com/uploads/image.jpg"
```

### CDN Providers

Works with any CDN that supports:

- **Cloudflare CDN**
- **AWS CloudFront**
- **Fastly**
- **Akamai**
- **Custom CDN**

## File Organization

### Recommended Structure

Organize files by type and date:

```
uploads/
├── avatars/
│   ├── user_123.jpg
│   └── user_456.jpg
├── documents/
│   ├── 2024/
│   │   ├── 01/
│   │   └── 02/
│   └── 2024/
├── images/
│   └── posts/
│       └── post_789.png
└── temp/
    └── upload_abc123.tmp
```

### Naming Conventions

- Use descriptive names
- Include timestamps for uniqueness
- Use consistent extensions
- Avoid special characters

Example:
```
avatars/user_123_20240115_120000.jpg
documents/report_20240115.pdf
temp/upload_abc123.tmp
```

## Security

### File Validation

Always validate uploaded files:

```rust
// Check file type
let content_type = get_content_type(&bytes);
if !ALLOWED_TYPES.contains(&content_type) {
    return Err("Invalid file type".into());
}

// Check file size
if bytes.len() > MAX_FILE_SIZE {
    return Err("File too large".into());
}

// Sanitize filename
let safe_name = sanitize_filename(&original_name);
```

### Path Traversal Prevention

The storage layer prevents path traversal by:

- Using `PathBuf::join()` which handles path components safely
- Not allowing `..` in paths
- Validating paths before operations

### Access Control

Implement access control at the application layer:

```rust
// Check user permissions before allowing download
if !user.can_download(&file_path) {
    return Err("Access denied".into());
}

let data = storage.download(&file_path).await?;
```

## Performance

### Local Filesystem

- **Latency**: ~1-5ms for local operations
- **Throughput**: Limited by disk I/O
- **Scalability**: Single-instance only
- **Cost**: Free (uses existing disk)

### S3 Storage

- **Latency**: ~50-200ms for S3 operations
- **Throughput**: High (parallel uploads)
- **Scalability**: Unlimited
- **Cost**: Pay per usage

### Optimization Tips

1. **Batch Operations**: Upload multiple files in parallel
2. **Use CDN**: Serve files through CDN for better performance
3. **Compress Files**: Compress large files before upload
4. **Cache**: Cache frequently accessed files
5. **Lazy Loading**: Load files only when needed

## Backup and Recovery

### Local Filesystem

```bash
# Backup
rsync -av /opt/axum-kickoff/uploads/ /backup/uploads/

# Restore
rsync -av /backup/uploads/ /opt/axum-kickoff/uploads/
```

### S3 Storage

S3 provides built-in:

- **Versioning**: Keep multiple versions of files
- **Cross-Region Replication**: Automatic replication
- **Lifecycle Policies**: Automatic archival/deletion
- **Backup**: Point-in-time recovery

## Migration

### Local to S3

To migrate from local to S3:

1. Configure S3 backend
2. Upload existing files to S3
3. Update CDN configuration
4. Switch backend in configuration
5. Verify all files are accessible

### Migration Script

```bash
#!/bin/bash
# Migrate local files to S3

aws s3 sync /opt/axum-kickoff/uploads/ s3://your-bucket/uploads/ --acl private
```

## Testing

### Unit Tests

The storage layer includes comprehensive unit tests:

```bash
cargo test storage
```

### Test Configuration

Use temporary directories for testing:

```rust
use tempfile::TempDir;

let temp_dir = TempDir::new()?;
let config = StorageConfig::local_filesystem(temp_dir.path());
let storage = Storage::from_config(&config);
```

## Troubleshooting

### Permission Denied

Ensure the storage directory is writable:

```bash
chmod 755 /opt/axum-kickoff/uploads
chown axum-kickoff:axum-kickoff /opt/axum-kickoff/uploads
```

### Disk Full

Check disk space:

```bash
df -h /opt/axum-kickoff/uploads
```

Clean up old files if necessary.

### File Not Found

- Verify the file path is correct
- Check file exists with `storage.exists()`
- Review logs for errors

### Slow Uploads

- Check network bandwidth
- Verify disk I/O performance
- Consider parallel uploads for multiple files

## Best Practices

### DO

- Validate file types and sizes before upload
- Use descriptive file names
- Organize files in directories
- Implement access control
- Use CDN for production
- Monitor storage usage
- Set up backups
- Clean up temporary files

### DON'T

- Don't store sensitive data unencrypted
- Don't allow arbitrary file paths
- Don't ignore file size limits
- Don't skip validation
- Don't use production storage for testing

## See Also

- [Configuration Documentation](CONFIGURATION.md#storage-configuration)
- [Deployment Documentation](DEPLOYMENT.md)
- [Architecture Documentation](ARCHITECTURE.md#storage-layer)

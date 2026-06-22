# Database Guide

This guide covers database operations in axum-kickoff using the Toasty ORM. Toasty is a modern Rust ORM that provides type-safe query builders, automatic migrations, and compile-time guarantees.

## Table of Contents

- [Overview](#overview)
- [Why Toasty?](#why-toasty)
- [Defining Models](#defining-models)
- [Database Configuration](#database-configuration)
- [Migrations](#migrations)
- [Querying Data](#querying-data)
- [Relationships](#relationships)
- [Raw SQL](#raw-sql)
- [Database Backends](#database-backends)
- [Testing](#testing)
- [Known Limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)

## Overview

axum-kickoff uses Toasty ORM for database operations with the following characteristics:

- **Default backend**: SQLite (zero-setup for development)
- **Production backend**: PostgreSQL (migration path available)
- **Schema definition**: Rust structs with `#[derive(toasty::Model)]`
- **Migrations**: Automatic generation from model changes
- **Type safety**: Compile-time checked queries
- **Connection pooling**: Built-in pooling with sensible defaults

## Why Toasty?

Toasty was chosen over more established ORMs like Diesel for several reasons:

- **Ergonomic API**: More intuitive than Diesel's macro-heavy approach
- **Automatic migrations**: No manual SQL migration files required
- **Async-first**: Built for Tokio and async workloads
- **Type-safe queries**: Compile-time guarantees for query correctness
- **Zero-cost abstractions**: Minimal runtime overhead

**Trade-off**: Toasty is newer and has a smaller community than Diesel. This documentation aims to compensate by being comprehensive.

## Defining Models

Models are defined as Rust structs in `src/models/`. Each model represents a database table.

### Basic Model Example

```rust
use toasty::Model;

#[derive(Debug, Model)]
pub struct User {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// GitHub account ID (unique identifier from GitHub)
    #[unique]
    pub gh_id: i64,

    /// GitHub username (login)
    pub gh_login: String,

    /// User's display name (from GitHub profile)
    pub name: Option<String>,

    /// Timestamp when the user was created
    pub created_at: jiff::Timestamp,
}
```

### Model Attributes

- `#[key]` - Marks the primary key field
- `#[auto]` - Auto-generates the field value (typically for IDs)
- `#[unique]` - Adds a unique constraint to the field
- `#[default(value)]` - Sets a default value for the field

### Field Types

Toasty supports common Rust types:

- **Integers**: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`
- **Strings**: `String`
- **Optionals**: `Option<T>` for nullable fields
- **Booleans**: `bool`
- **Timestamps**: `jiff::Timestamp` (recommended) or `chrono::DateTime`
- **Floats**: `f32`, `f64`

### Adding a New Model

1. Create a new file in `src/models/` (e.g., `src/models/product.rs`)
2. Define your model struct with `#[derive(toasty::Model)]`
3. Add the module to `src/models/mod.rs`:

```rust
pub mod product;
pub use product::Product;
```

4. Generate a migration:

```bash
just migration-generate
```

5. Apply the migration:

```bash
just migration-apply
```

See [ADD_NEW_MODEL.md](ADD_NEW_MODEL.md) for a detailed walkthrough.

## Database Configuration

### Connection String

The database connection is configured via the `DATABASE_URL` environment variable:

```bash
# SQLite (file-based, default)
DATABASE_URL=sqlite:./axum_kickoff.db

# SQLite (in-memory, for testing)
DATABASE_URL=sqlite::memory:

# PostgreSQL
DATABASE_URL=postgresql://user:password@localhost:5432/axum_kickoff
```

### Connection Pooling

The database connection pool is configured in `src/db.rs` with sensible defaults:

- **max_pool_size**: `num_cpus * 2`
- **pool_wait_timeout**: 5 seconds
- **pool_create_timeout**: 10 seconds
- **pool_health_check_interval**: 60 seconds
- **pool_pre_ping**: true (checks connection health before use)

You can customize pool settings by modifying `Database::from_config()` in `src/db.rs`.

## Migrations

Toasty automatically generates migrations based on model changes. No manual SQL migration files are required.

### Migration Workflow

1. **Make model changes** - Add or modify models in `src/models/`
2. **Generate migration** - Run `just migration-generate`
3. **Review migration** - Check the generated SQL in `migrations/`
4. **Apply migration** - Run `just migration-apply`

### Migration Commands

```bash
# Generate a new migration based on model changes
just migration-generate

# Apply pending migrations to the database
just migration-apply

# Create a schema snapshot for future migration generation
just migration-snapshot

# Drop the last migration file
just migration-drop

# Reset the database - WARNING: This will delete all data
just migration-reset

# Inspect the current database schema as SQL
just migration-inspect
```

### Migration Files

Generated migrations are stored in the `migrations/` directory with sequential naming:

```
migrations/
├── 0001_initial.sql
├── 0002_add_user_index.sql
└── 0003_add_api_tokens.sql
```

### Toasty Configuration

Migration behavior is configured in `Toasty.toml`:

```toml
[migration]
path = "migrations"
prefix_style = "Sequential"
checksums = false
statement_breakpoints = true
```

- `path`: Directory for migration files
- `prefix_style`: Naming convention for migrations
- `checksums`: Enable migration checksums (currently disabled)
- `statement_breakpoints`: Add breakpoints between SQL statements

## Querying Data

Toasty provides a fluent, type-safe query API.

### Basic Queries

```rust
use crate::models::User;

// Find by ID
let user = User::get(id).exec(&db).await?;

// Find by unique field
let user = User::filter(User::GhId.eq(gh_id)).get(&db).await?;

// Find all records
let users = User::all().exec(&db).await?;

// Count records
let count = User::all().count(&db).await?;
```

### Filtering

```rust
// Single condition
let active_users = User::filter(User::IsActive.eq(true))
    .exec(&db)
    .await?;

// Multiple conditions
let recent_users = User::filter(User::IsActive.eq(true))
    .filter(User::CreatedAt.gt(some_timestamp))
    .exec(&db)
    .await?;

// Complex conditions
let users = User::filter(
    User::IsActive.eq(true)
        .and(User::GhLogin.like("admin%"))
)
.exec(&db)
.await?;
```

### Sorting

```rust
// Sort ascending
let users = User::all()
    .sort(User::CreatedAt.asc())
    .exec(&db)
    .await?;

// Sort descending
let users = User::all()
    .sort(User::CreatedAt.desc())
    .exec(&db)
    .await?;

// Multiple sort fields
let users = User::all()
    .sort(User::IsActive.desc())
    .sort(User::CreatedAt.asc())
    .exec(&db)
    .await?;
```

### Limiting and Pagination

```rust
// Limit results
let users = User::all()
    .limit(10)
    .exec(&db)
    .await?;

// Offset for pagination
let page2 = User::all()
    .limit(10)
    .offset(10)
    .exec(&db)
    .await?;
```

### Creating Records

```rust
use crate::models::User;

let user = User::new_from_github(gh_id, gh_login, name, email, avatar);
user.insert(&db).await?;
```

### Updating Records

```rust
let mut user = User::get(id).exec(&db).await?;
user.name = Some("New Name".to_string());
user.touch(); // Update timestamp
user.update(&db).await?;
```

### Deleting Records

```rust
let user = User::get(id).exec(&db).await?;
user.delete(&db).await?;
```

## Relationships

Toasty supports relationships between models.

### Defining Relationships

```rust
#[derive(Debug, Model)]
pub struct Post {
    #[key]
    #[auto]
    pub id: u64,

    pub title: String,
    pub content: String,

    // Foreign key to User
    pub user_id: u64,
}

#[derive(Debug, Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: u64,

    pub gh_login: String,
}
```

### Querying Relationships

```rust
// Get all posts by a user
let posts = Post::filter(Post::UserId.eq(user_id))
    .exec(&db)
    .await?;

// Join with user (if relationship is defined)
// Note: Toasty's relationship API is evolving; check latest docs
```

## Raw SQL

When Toasty's query builder isn't sufficient, you can execute raw SQL.

### Executing Raw SQL

```rust
use toasty::sql;

// Execute a raw SQL statement
sql::query("UPDATE users SET is_active = false WHERE id = ?", [user_id])
    .exec(&db)
    .await?;

// Execute a raw query and parse results
let rows = sql::query("SELECT * FROM users WHERE gh_id = ?", [gh_id])
    .fetch_all(&db)
    .await?;
```

### Using SQLite Directly

For SQLite-specific operations, you can access the underlying connection:

```rust
use crate::db::Database;

// This requires accessing the underlying Toasty Db
// and then the raw SQLite connection
// Implementation depends on Toasty's internal API
```

**Note**: Raw SQL should be used sparingly, as it bypasses Toasty's type safety and migration system.

## Database Backends

### SQLite (Default)

SQLite is the default database for development:

- **Pros**: Zero-setup, embedded, file-based, great for development
- **Cons**: Not suitable for high-concurrency production workloads
- **Use case**: Development, testing, low-traffic applications

#### SQLite Operations

```bash
# View database contents
sqlite3 axum_kickoff.db

# Backup database
cp axum_kickoff.db axum_kickoff.db.backup

# Reset database (delete file)
rm axum_kickoff.db
```

#### SQLite in Testing

For tests, use in-memory SQLite:

```bash
DATABASE_URL=sqlite::memory: cargo test
```

Or set it in your test environment:

```rust
#[tokio::test]
async fn test_something() {
    let db_url = "sqlite::memory:";
    // ... test code
}
```

### PostgreSQL (Production)

PostgreSQL is recommended for production deployments:

- **Pros**: High concurrency, advanced features, battle-tested
- **Cons**: Requires separate database server
- **Use case**: Production, high-traffic applications

#### Switching to PostgreSQL

1. Install PostgreSQL:

```bash
# Ubuntu/Debian
sudo apt-get install postgresql postgresql-contrib

# macOS
brew install postgresql
brew services start postgresql
```

2. Create a database:

```bash
createdb axum_kickoff
```

3. Update `.env`:

```bash
DATABASE_URL=postgresql://username:password@localhost:5432/axum_kickoff
```

4. Run migrations:

```bash
just migration-apply
```

#### PostgreSQL Connection Pooling

For production, consider using PgBouncer for connection pooling:

```bash
# Install PgBouncer
sudo apt-get install pgbouncer

# Configure PgBouncer
# Edit /etc/pgbouncer/pgbouncer.ini
```

## Testing

### Test Database Setup

Tests use a temporary SQLite database:

```rust
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_user_creation() {
    let db_file = NamedTempFile::new().expect("Failed to create temp database");
    let db_url = format!("sqlite:{}", db_file.path().display());

    let config = DatabaseConfig { url: db_url };
    let db = Database::from_config(&config).await.unwrap();

    // ... test code
}
```

### Test Utilities

The project provides test utilities in `src/tests/`:

- `test_app.rs` - Test application builder
- `builders.rs` - Model builders for test data
- `request_helper.rs` - HTTP request helpers

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_user_creation
```

## Known Limitations

Toasty is a relatively new ORM with some limitations compared to more mature ORMs:

### Limited Relationship Support

- Complex relationships (many-to-many, polymorphic) may require workarounds
- Eager loading (N+1 problem prevention) is not as mature as other ORMs

### Migration Limitations

- Automatic migrations may not handle all schema changes
- Complex refactoring (renaming tables, changing column types) may require manual intervention
- Rollback support is limited

### Query Builder Limitations

- Some complex SQL queries may not be expressible in the query builder
- Subqueries and window functions have limited support
- Custom aggregations may require raw SQL

### Documentation

- Toasty's documentation is less comprehensive than Diesel's
- Community knowledge and examples are limited

### Performance

- Performance characteristics are less well-documented
- Query optimization may require manual tuning

**Mitigation**: For complex queries or relationships, use raw SQL or consider switching to Diesel for those specific use cases.

## Troubleshooting

### Migration Fails

**Problem**: Migration generation or application fails.

**Solutions**:

1. Check that your model definitions are valid Rust code
2. Ensure all referenced types are in scope
3. Verify that `Toasty.toml` is correctly configured
4. Try running `just migration-reset` to start fresh (WARNING: deletes data)

### Connection Pool Exhausted

**Problem**: "Connection pool exhausted" error.

**Solutions**:

1. Increase `max_pool_size` in `src/db.rs`
2. Check for connection leaks (ensure connections are released)
3. Reduce concurrent database operations

### SQLite Lock Errors

**Problem**: "database is locked" errors in SQLite.

**Solutions**:

1. SQLite has limited write concurrency; consider PostgreSQL for high-write workloads
2. Ensure transactions are short-lived
3. Use WAL mode: `PRAGMA journal_mode=WAL;`

### Type Mismatch Errors

**Problem**: Type mismatch between model and database.

**Solutions**:

1. Run `just migration-apply` to ensure schema is up-to-date
2. Check that field types match between model and migration
3. For timestamp issues, ensure consistent use of `jiff::Timestamp`

### Query Compilation Errors

**Problem**: Query builder fails to compile.

**Solutions**:

1. Ensure all field references use the correct model (e.g., `User::GhId`)
2. Check that filter conditions are properly typed
3. Review Toasty's query builder API documentation

## Additional Resources

- [Toasty GitHub Repository](https://github.com/stepchowfun/toasty)
- [Toasty Documentation](https://github.com/stepchowfun/toasty#readme)
- [SQLite Documentation](https://www.sqlite.org/docs.html)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [Jiff (Time Library)](https://github.com/Brendonovich/jiff)

## Migration from Other ORMs

If you're migrating from another ORM (e.g., Diesel):

1. **Define models** - Recreate your models using Toasty's syntax
2. **Generate migrations** - Let Toasty generate the initial schema
3. **Migrate data** - Use raw SQL to migrate existing data
4. **Update queries** - Rewrite queries using Toasty's query builder
5. **Test thoroughly** - Ensure behavior matches the original implementation

For specific migration help, see the [Toasty documentation](https://github.com/stepchowfun/toasty) or open an issue.

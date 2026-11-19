//! Optional PostgreSQL test database harness
//!
//! By default tests use a temporary SQLite database file. Set
//! `TEST_DATABASE_URL` to point at a PostgreSQL database to run the suite
//! against the real backend.
//!
//! For PostgreSQL, each test creates its own `t_<uuid>` schema and points the
//! connection's `search_path` at it, so parallel tests in one process stay
//! fully isolated on a single shared database. Schemas are dropped when the
//! test's `TestDatabase` goes out of scope.

use crate::config::DatabaseConfig;
use crate::db::Database;
use secrecy::SecretString;
use tempfile::NamedTempFile;
use toasty::sql;

/// Test database handle that keeps any temporary file alive for the
/// duration of the test.
pub struct TestDatabase {
    pub config: DatabaseConfig,
    _temp_file: Option<NamedTempFile>,
    schema: Option<TestSchema>,
}

/// Drop guard for a per-test PostgreSQL schema: removes the schema when the
/// owning `TestDatabase` is dropped.
struct TestSchema {
    /// The original `TEST_DATABASE_URL`, without `search_path` injection.
    base_url: String,
    name: String,
}

impl Drop for TestSchema {
    fn drop(&mut self) {
        // A fresh thread: `Drop` runs inside the test's tokio runtime where
        // `block_on` would panic, and a detached `tokio::spawn` would be
        // killed by runtime shutdown. Joining keeps cleanup deterministic.
        let url = self.base_url.clone();
        let name = self.name.clone();
        let _ = std::thread::Builder::new()
            .name("test-schema-drop".into())
            .spawn(move || {
                let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                rt.block_on(async move {
                    let config = DatabaseConfig {
                        url: SecretString::from(url),
                    };
                    if let Ok(db) = Database::from_config(&config).await {
                        let _ = sql::statement(format!("DROP SCHEMA IF EXISTS \"{name}\" CASCADE"))
                            .exec(&mut db.db_clone())
                            .await;
                    }
                });
            })
            .map(|h| h.join());
    }
}

impl Default for TestDatabase {
    fn default() -> Self {
        Self::new()
    }
}

/// True for URLs the PostgreSQL driver handles (`postgres:`/`postgresql:`).
fn is_postgres(url: &str) -> bool {
    url.starts_with("postgres://") || url.starts_with("postgresql://")
}

/// `url` with `search_path` appended to its `options` parameter, so every
/// connection resolves unqualified table names inside `schema`.
fn url_with_search_path(url: &str, schema: &str) -> String {
    let Ok(mut url) = url::Url::parse(url) else {
        return url.to_string();
    };
    let existing = url
        .query_pairs()
        .find(|(k, _)| k == "options")
        .map(|(_, v)| v.into_owned());
    // `-c` assignments apply in order; ours goes last so a user-provided
    // search_path can't silently defeat test isolation.
    let options = match existing {
        Some(o) => format!("{o} -c search_path={schema}"),
        None => format!("-c search_path={schema}"),
    };
    url.query_pairs_mut().append_pair("options", &options);
    url.to_string()
}

impl TestDatabase {
    /// Create a new test database configuration.
    ///
    /// If `TEST_DATABASE_URL` is set, it is used as-is (with per-test schema
    /// isolation for PostgreSQL). Otherwise a temporary SQLite file is
    /// created.
    pub fn new() -> Self {
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            let mut schema = None;
            let mut connect_url = url.clone();
            if is_postgres(&url) {
                let name = format!("t{}", uuid::Uuid::new_v4().simple());
                connect_url = url_with_search_path(&url, &name);
                schema = Some(TestSchema {
                    base_url: url,
                    name,
                });
            }
            return Self {
                config: DatabaseConfig {
                    url: SecretString::from(connect_url),
                },
                _temp_file: None,
                schema,
            };
        }

        let db_file = NamedTempFile::new().expect("Failed to create temp database file");
        let db_url = format!("sqlite:{}", db_file.path().display());

        Self {
            config: DatabaseConfig {
                url: SecretString::from(db_url),
            },
            _temp_file: Some(db_file),
            schema: None,
        }
    }

    /// Connect to the database and create the schema.
    ///
    /// Uses `Db::push_schema` rather than applying the checked-in migration
    /// files: the migration set is generated for the configured app backend
    /// (see `database` in `cargo-generate.toml`), while tests may run on a
    /// different backend (SQLite temp files by default). `push_schema`
    /// creates tables through the connected driver, so it is dialect-agnostic.
    pub async fn connect_and_migrate(&self) -> Database {
        let db = Database::from_config(&self.config)
            .await
            .expect("Failed to connect to test database");

        let mut dbc = db.db_clone();
        if let Some(schema) = &self.schema {
            sql::statement(format!("CREATE SCHEMA \"{}\"", schema.name))
                .exec(&mut dbc)
                .await
                .expect("Failed to create test schema");
        }
        dbc.push_schema()
            .await
            .expect("Failed to create test database schema");

        db
    }
}

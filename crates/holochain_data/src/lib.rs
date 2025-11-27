//! A wrapper around sqlx, configured for use in Holochain.
//!
//! This crate provides a configured SQLite connection pool for use in Holochain.

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Pool, Sqlite,
};
use std::path::Path;
#[cfg(feature = "test-utils")]
use std::str::FromStr;

mod key;
pub use key::DbKey;

pub mod example;
mod handles;
pub use handles::{DbRead, DbWrite};
pub mod kind;
pub mod models;
pub mod wasm;

/// Embedded migrations compiled into the binary.
///
/// This macro embeds all SQL files from the `migrations/` directory at compile time.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// SQLite synchronous level configuration.
///
/// Corresponds to the `PRAGMA synchronous` pragma.
/// See [sqlite documentation](https://www.sqlite.org/pragma.html#pragma_synchronous).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DbSyncLevel {
    /// Use xSync for all writes. Not needed for WAL mode.
    Full,
    /// Sync at critical moments. Default.
    #[default]
    Normal,
    /// Syncing is left to the operating system and power loss could result in corrupted database.
    Off,
}

impl DbSyncLevel {
    fn as_pragma_value(&self) -> &str {
        match self {
            DbSyncLevel::Full => "2",
            DbSyncLevel::Normal => "1",
            DbSyncLevel::Off => "0",
        }
    }
}

/// Configuration options for Holochain database connections.
#[derive(Debug, Clone, Default)]
pub struct HolochainDataConfig {
    /// Optional encryption key for the database.
    pub key: Option<DbKey>,
    /// SQLite synchronous level.
    pub sync_level: DbSyncLevel,
}

impl HolochainDataConfig {
    /// Create a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the encryption key.
    pub fn with_key(mut self, key: DbKey) -> Self {
        self.key = Some(key);
        self
    }

    /// Set the synchronous level.
    pub fn with_sync_level(mut self, sync_level: DbSyncLevel) -> Self {
        self.sync_level = sync_level;
        self
    }
}

pub trait DatabaseIdentifier: Clone {
    fn database_id(&self) -> &str;
}

/// Open a database connection at the given directory path.
///
/// The database file name is constructed from the `database_id`.
///
/// # Errors
///
/// Returns an error if `path` is not a directory.
pub async fn setup_holochain_data<I: DatabaseIdentifier>(
    path: impl AsRef<Path>,
    database_id: I,
    config: HolochainDataConfig,
) -> sqlx::Result<DbWrite<I>> {
    let path = path.as_ref();
    if !path.is_dir() {
        return Err(sqlx::Error::Configuration(
            format!("Path must be a directory: {}", path.display()).into(),
        ));
    }

    let db_file = path.join(database_id.database_id());
    let pool = connect_database(&db_file, config).await?;

    // Run migrations
    MIGRATOR.run(&pool).await?;

    Ok(DbWrite::new(pool, database_id))
}

#[cfg(feature = "test-utils")]
pub async fn test_setup_holochain_data<I: DatabaseIdentifier>(
    database_id: I,
) -> sqlx::Result<DbWrite<I>> {
    let pool = connect_database_memory(HolochainDataConfig::default()).await?;

    // Run migrations
    MIGRATOR.run(&pool).await?;

    Ok(DbWrite::new(pool, database_id))
}

/// Connect to a SQLite database using the provided connection string.
async fn connect_database(
    db_path: &Path,
    config: HolochainDataConfig,
) -> sqlx::Result<Pool<Sqlite>> {
    let mut opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);

    opts = configure_sqlite_options(opts, config)?;

    create_pool(opts).await
}

/// Connect to an in-memory SQLite database for testing.
#[cfg(feature = "test-utils")]
async fn connect_database_memory(config: HolochainDataConfig) -> sqlx::Result<Pool<Sqlite>> {
    let opts = SqliteConnectOptions::from_str(":memory:")?;
    let opts = configure_sqlite_options(opts, config)?;

    create_pool(opts).await
}

/// Configure SQLite-specific options including encryption and WAL mode.
fn configure_sqlite_options(
    mut opts: SqliteConnectOptions,
    config: HolochainDataConfig,
) -> sqlx::Result<SqliteConnectOptions> {
    // Apply encryption pragmas if key is provided
    if let Some(ref key) = config.key {
        opts = key.apply_pragmas(opts);
    }

    // Always enable WAL mode for better concurrency
    opts = opts.journal_mode(SqliteJournalMode::Wal);

    // Set other pragmas
    let sync_value = config.sync_level.as_pragma_value().to_string();
    opts = opts
        .pragma("trusted_schema", "false")
        .pragma("foreign_keys", "ON")
        .pragma("synchronous", sync_value);

    Ok(opts)
}

/// Create a connection pool with standard options.
async fn create_pool(opts: SqliteConnectOptions) -> sqlx::Result<Pool<Sqlite>> {
    let max_cons = num_read_threads();
    let pool = SqlitePoolOptions::new()
        .max_connections(max_cons as u32)
        .min_connections(0)
        .idle_timeout(std::time::Duration::from_secs(30))
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect_with(opts)
        .await?;

    Ok(pool)
}

/// Calculate the number of read threads based on CPU count.
///
/// Returns at least 4, or the number of CPUs.
fn num_read_threads() -> usize {
    let num_cpus = num_cpus::get();
    std::cmp::max(num_cpus, 4)
}

#[cfg(all(test, feature = "test-utils"))]
mod tests {
    use super::*;
    use sqlx::Row;

    #[derive(Debug, Clone)]
    struct TestDbId;

    impl DatabaseIdentifier for TestDbId {
        fn database_id(&self) -> &str {
            "test_db"
        }
    }

    #[tokio::test]
    async fn test_in_memory_database_with_migrations() {
        // Set up in-memory database
        let db = test_setup_holochain_data(TestDbId)
            .await
            .expect("Failed to set up test database");

        // Verify migrations ran by checking the sample_data table exists
        let row = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='Wasm'")
            .fetch_one(db.pool())
            .await
            .expect("Failed to query sqlite_master");

        let table_name: String = row.get(0);
        assert_eq!(table_name, "Wasm");

        // Test inserting and querying data
        sqlx::query("INSERT INTO sample_data (name, value) VALUES (?, ?)")
            .bind("test_name")
            .bind("test_value")
            .execute(db.pool())
            .await
            .expect("Failed to insert data");

        let row = sqlx::query("SELECT name, value FROM sample_data WHERE name = ?")
            .bind("test_name")
            .fetch_one(db.pool())
            .await
            .expect("Failed to query data");

        let name: String = row.get(0);
        let value: Option<String> = row.get(1);
        assert_eq!(name, "test_name");
        assert_eq!(value, Some("test_value".to_string()));
    }
}

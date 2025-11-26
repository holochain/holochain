//! A wrapper around SeaORM, configured for use in Holochain.
//!
//! This crate provides a configured SQLite connection pool for use in Holochain.

use std::path::Path;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Error as SqlxError, Pool, Sqlite,
};

mod key;
pub use key::DbKey;

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
pub struct HolochainOrmConfig {
    /// Optional encryption key for the database.
    pub key: Option<DbKey>,
    /// SQLite synchronous level.
    pub sync_level: DbSyncLevel,
}

impl HolochainOrmConfig {
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

pub trait DatabaseIdentifier {
    fn database_id(&self) -> &str;
}

pub struct HolochainDbConn<I: DatabaseIdentifier> {
    pub pool: Pool<Sqlite>,
    pub identifier: I,
}

/// Open a database connection at the given directory path.
///
/// The database file name is constructed from the `database_id`.
///
/// # Errors
///
/// Returns an error if `path` is not a directory.
pub async fn setup_holochain_orm<I: DatabaseIdentifier>(
    path: impl AsRef<Path>,
    database_id: I,
    config: HolochainOrmConfig,
) -> Result<HolochainDbConn<I>, SqlxError> {
    let path = path.as_ref();
    if !path.is_dir() {
        return Err(SqlxError::Configuration(
            format!("Path must be a directory: {}", path.display()).into(),
        ));
    }

    let db_file = path.join(database_id.database_id());
    let pool = connect_database(&db_file, config).await?;

    // Run migrations
    MIGRATOR.run(&pool).await?;

    Ok(HolochainDbConn {
        pool,
        identifier: database_id,
    })
}

#[cfg(feature = "test-utils")]
pub async fn test_setup_holochain_orm<I: DatabaseIdentifier>(
    database_id: I,
) -> Result<HolochainDbConn<I>, SqlxError> {
    let pool = connect_database_memory(HolochainOrmConfig::default()).await?;
    Ok(HolochainDbConn {
        pool,
        identifier: database_id,
    })
}

/// Connect to a SQLite database using the provided connection string.
async fn connect_database(
    db_path: &Path,
    config: HolochainOrmConfig,
) -> Result<Pool<Sqlite>, SqlxError> {
    let mut opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);

    opts = configure_sqlite_options(opts, config)?;

    create_pool(opts).await
}

/// Connect to an in-memory SQLite database for testing.
#[cfg(feature = "test-utils")]
async fn connect_database_memory(
    config: HolochainOrmConfig,
) -> Result<Pool<Sqlite>, SqlxError> {
    let opts = SqliteConnectOptions::from_str(":memory:")?;
    let opts = configure_sqlite_options(opts, config)?;

    create_pool(opts).await
}

/// Configure SQLite-specific options including encryption and WAL mode.
fn configure_sqlite_options(
    mut opts: SqliteConnectOptions,
    config: HolochainOrmConfig,
) -> Result<SqliteConnectOptions, SqlxError> {
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
        .pragma("synchronous", sync_value);

    Ok(opts)
}

/// Create a connection pool with standard options.
async fn create_pool(opts: SqliteConnectOptions) -> Result<Pool<Sqlite>, SqlxError> {
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

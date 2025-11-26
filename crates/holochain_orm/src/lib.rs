//! A wrapper around SeaORM, configured for use in Holochain.
//!
//! This crate does not implement an ORM itself but provides what Holochain needs to use SeaORM.

use std::path::Path;
use sea_orm::{
    sqlx::sqlite::SqliteJournalMode, ConnectOptions, Database, DatabaseConnection, DbErr,
    RuntimeErr,
};

mod key;
pub use key::DbKey;

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
    pub conn: DatabaseConnection,
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
) -> Result<HolochainDbConn<I>, DbErr> {
    let path = path.as_ref();
    if !path.is_dir() {
        return Err(DbErr::Conn(RuntimeErr::Internal(
            format!("Path must be a directory: {}", path.display())
        )));
    }

    let db_file = path.join(database_id.database_id());
    let connection_string = format!("sqlite://{}?mode=rwc", db_file.display());
    let conn = connect_database(&connection_string, config).await?;

    Ok(HolochainDbConn {
        conn,
        identifier: database_id,
    })
}

#[cfg(feature = "test-utils")]
pub async fn test_setup_holochain_orm<I: DatabaseIdentifier>(
    database_id: I,
) -> Result<HolochainDbConn<I>, DbErr> {
    let connection_string = "sqlite::memory:".to_string();
    let conn = connect_database(&connection_string, None).await?;
    Ok(HolochainDbConn {
        conn,
        identifier: database_id,
    })
}

/// Connect to a SQLite database using the provided connection string.
async fn connect_database(
    connection_string: &str,
    config: HolochainOrmConfig,
) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(connection_string);

    // Configure connection pool:
    // SeaORM handles read/write connections internally, so we just need
    // a reasonable pool size based on CPU count.
    let max_cons = num_read_threads();

    opt.max_connections(max_cons as u32)
        .min_connections(0)
        .idle_timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(30));

    let sync_value = config.sync_level.as_pragma_value().to_string();

    // Configure SQLite-specific options including encryption and WAL mode
    opt.map_sqlx_sqlite_opts(move |mut opts| {
        // Apply encryption pragmas if key is provided
        if let Some(ref key) = config.key {
            opts = key.apply_pragmas(opts);
        }
        // Always enable WAL mode for better concurrency and set other pragmas
        opts.journal_mode(SqliteJournalMode::Wal)
            .pragma("trusted_schema", "false")
            .pragma("synchronous", sync_value.clone())
    });

    Database::connect(opt).await
}

/// Calculate the number of read threads based on CPU count.
///
/// Returns at least 4, or the number of CPUs.
fn num_read_threads() -> usize {
    let num_cpus = num_cpus::get();
    std::cmp::max(num_cpus, 4)
}

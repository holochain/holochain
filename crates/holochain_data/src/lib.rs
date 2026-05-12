//! A wrapper around sqlx, configured for use in Holochain.
//!
//! This crate provides a configured SQLite connection pool for use in Holochain.

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Pool, Sqlite,
};
use std::path::Path;
#[cfg(any(test, feature = "test-utils"))]
use std::str::FromStr;

mod key;
pub use key::DbKey;

pub mod example;
mod handles;
pub use handles::{DbRead, DbWrite, TxRead, TxWrite};
pub mod conductor;
pub mod dht;
pub mod kind;
pub mod models;
pub mod peer_meta_store;
pub mod wasm;

/// Embedded migrations for the Wasm database.
static WASM_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/wasm");

/// Embedded migrations for the Conductor database.
static CONDUCTOR_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/conductor");

/// Embedded migrations for the [`kind::DbKind::PeerMetaStore`] database.
static PEER_META_STORE_MIGRATOR: sqlx::migrate::Migrator =
    sqlx::migrate!("./migrations/peer_meta_store");

/// Embedded migrations for the DHT database.
static DHT_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/dht");

/// Select the appropriate migrator for a database kind.
fn migrator_for(db_kind: kind::DbKind) -> &'static sqlx::migrate::Migrator {
    match db_kind {
        kind::DbKind::Wasm => &WASM_MIGRATOR,
        kind::DbKind::Conductor => &CONDUCTOR_MIGRATOR,
        kind::DbKind::PeerMetaStore => &PEER_META_STORE_MIGRATOR,
        kind::DbKind::Dht => &DHT_MIGRATOR,
    }
}

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
#[derive(Debug, Clone)]
pub struct HolochainDataConfig {
    /// Optional encryption key for the database.
    pub key: Option<DbKey>,
    /// SQLite synchronous level.
    pub sync_level: DbSyncLevel,
    /// Number of read connections in the pool.
    pub max_readers: u16,
}

impl Default for HolochainDataConfig {
    fn default() -> Self {
        Self {
            key: None,
            sync_level: Default::default(),
            max_readers: 8,
        }
    }
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

    /// Set the number of read connections in the pool
    pub fn with_max_readers(mut self, max_readers: u16) -> Self {
        self.max_readers = max_readers;
        self
    }
}

/// Identifies a specific database file and the schema it expects.
///
/// Implementors pair a unique filename ([`database_id`](Self::database_id))
/// with a schema kind ([`db_kind`](Self::db_kind)) — the pair must stay in
/// sync, since `db_kind` is what [`open_db`] uses to pick the migration set
/// applied to the file.
pub trait DatabaseIdentifier: Clone {
    /// The stable filename for this database, relative to the databases
    /// directory.
    fn database_id(&self) -> &str;

    /// The schema kind for this database.
    ///
    /// Controls which migration set is applied when the database is opened,
    /// so this must match the schema expected at
    /// [`database_id`](Self::database_id).
    fn db_kind(&self) -> kind::DbKind;
}

#[derive(Debug)]
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
pub async fn open_db<I: DatabaseIdentifier>(
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

    // Run migrations for this database kind
    migrator_for(database_id.db_kind()).run(&pool).await?;

    Ok(DbWrite::new(pool, database_id))
}

#[cfg(any(test, feature = "test-utils"))]
pub async fn test_open_db<I: DatabaseIdentifier>(database_id: I) -> sqlx::Result<DbWrite<I>> {
    let pool = connect_database_memory(HolochainDataConfig::default()).await?;

    // Run migrations for this database kind
    migrator_for(database_id.db_kind()).run(&pool).await?;

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

    let max_readers = config.max_readers;
    opts = configure_sqlite_options(opts, config)?;

    create_pool(opts, max_readers).await
}

/// Connect to an in-memory SQLite database for testing.
#[cfg(any(test, feature = "test-utils"))]
async fn connect_database_memory(config: HolochainDataConfig) -> sqlx::Result<Pool<Sqlite>> {
    let opts = SqliteConnectOptions::from_str(":memory:")?;
    let max_readers = config.max_readers;
    let opts = configure_sqlite_options(opts, config)?;

    create_pool(opts, max_readers).await
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
async fn create_pool(opts: SqliteConnectOptions, max_readers: u16) -> sqlx::Result<Pool<Sqlite>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(max_readers as u32 + 1)
        .min_connections(0)
        .idle_timeout(std::time::Duration::from_secs(30))
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect_with(opts)
        .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    use kind::DbKind;

    #[derive(Debug, Clone)]
    struct TestWasmDbId;

    impl DatabaseIdentifier for TestWasmDbId {
        fn database_id(&self) -> &str {
            "test_wasm_db"
        }

        fn db_kind(&self) -> DbKind {
            DbKind::Wasm
        }
    }

    #[derive(Debug, Clone)]
    struct TestConductorDbId;

    impl DatabaseIdentifier for TestConductorDbId {
        fn database_id(&self) -> &str {
            "test_conductor_db"
        }

        fn db_kind(&self) -> DbKind {
            DbKind::Conductor
        }
    }

    #[tokio::test]
    async fn wasm_migrations_applied() {
        let db = test_open_db(TestWasmDbId)
            .await
            .expect("Failed to set up test database");

        // Verify Wasm tables exist
        let tables = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .expect("Failed to query sqlite_master");

        assert!(tables.contains(&"Wasm".to_string()));
        assert!(tables.contains(&"DnaDef".to_string()));
        assert!(tables.contains(&"IntegrityZome".to_string()));
        assert!(tables.contains(&"CoordinatorZome".to_string()));
        assert!(tables.contains(&"EntryDef".to_string()));

        // Verify Conductor tables do NOT exist
        assert!(!tables.contains(&"Conductor".to_string()));
        assert!(!tables.contains(&"InstalledApp".to_string()));
    }

    #[tokio::test]
    async fn conductor_migrations_applied() {
        let db = test_open_db(TestConductorDbId)
            .await
            .expect("Failed to set up test database");

        // Verify Conductor tables exist
        let tables = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .expect("Failed to query sqlite_master");

        assert!(tables.contains(&"Conductor".to_string()));
        assert!(tables.contains(&"InstalledApp".to_string()));
        assert!(tables.contains(&"AppRole".to_string()));
        assert!(tables.contains(&"Nonce".to_string()));
        assert!(tables.contains(&"BlockSpan".to_string()));

        // Verify Wasm tables do NOT exist
        assert!(!tables.contains(&"Wasm".to_string()));
        assert!(!tables.contains(&"DnaDef".to_string()));
    }

    #[derive(Debug, Clone)]
    struct TestDhtDbId;

    impl DatabaseIdentifier for TestDhtDbId {
        fn database_id(&self) -> &str {
            "test_dht_db"
        }

        fn db_kind(&self) -> DbKind {
            DbKind::Dht
        }
    }

    #[tokio::test]
    async fn dht_migrations_applied() {
        let db = test_open_db(TestDhtDbId)
            .await
            .expect("Failed to set up test database");

        let tables = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .expect("Failed to query sqlite_master");

        for expected in [
            "Action",
            "CapClaim",
            "CapGrant",
            "ChainLock",
            "ChainOp",
            "ChainOpPublish",
            "DeletedLink",
            "DeletedRecord",
            "Entry",
            "Link",
            "LimboChainOp",
            "LimboWarrant",
            "PrivateEntry",
            "ScheduledFunction",
            "UpdatedRecord",
            "ValidationReceipt",
            "Warrant",
            "WarrantPublish",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "missing table {expected}; have: {:?}",
                tables
            );
        }

        assert!(!tables.contains(&"Conductor".to_string()));
        assert!(!tables.contains(&"Wasm".to_string()));
    }
}

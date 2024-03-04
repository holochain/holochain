use crate::functions::add_custom_functions;
use holochain_serialized_bytes::prelude::*;
use once_cell::sync::Lazy;
use rusqlite::*;
use scheduled_thread_pool::ScheduledThreadPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{path::Path, sync::Arc, time::Duration};

// Should never be getting a connection from the pool when one isn't available so this can be set low
static CONNECTION_TIMEOUT_MS: AtomicU64 = AtomicU64::new(3_000);

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(feature = "sqlite-encrypted")]
pub(super) const FAKE_KEY: &str = "x'98483C6EB40B6C31A448C22A66DED3B5E5E8D5119CAC8327B655C8B5C483648101010101010101010101010101010101'";

static R2D2_THREADPOOL: Lazy<Arc<ScheduledThreadPool>> = Lazy::new(|| {
    let t = ScheduledThreadPool::new(1);
    Arc::new(t)
});

pub type ConnectionPool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

/// The sqlite synchronous level.
/// Corresponds to the `PRAGMA synchronous` pragma.
/// See [sqlite documentation](https://www.sqlite.org/pragma.html#pragma_synchronous).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
pub enum DbSyncLevel {
    /// Use xSync for all writes. Not needed for WAL mode.
    Full,
    /// Sync at critical moments. Default.
    #[default]
    Normal,
    /// Syncing is left to the operating system and power loss could result in corrupted database.
    Off,
}

/// The strategy for database file system synchronization.
/// Some databases like the cache can be safely rebuilt if
/// corruption occurs due to using the faster [`DbSyncLevel::Off`].
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
pub enum DbSyncStrategy {
    /// Allows databases that can be wiped and rebuilt to
    /// use the faster [`DbSyncLevel::Off`].
    /// This is the default.
    #[default]
    Fast,
    /// Makes all databases use at least [`DbSyncLevel::Normal`].
    /// This is probably not needed unless you have an SSD and
    /// would prefer to lower the chances of databases needing to
    /// be rebuilt.
    Resilient,
}

pub(super) fn new_connection_pool(
    path: Option<&Path>,
    synchronous_level: DbSyncLevel,
) -> ConnectionPool {
    use r2d2_sqlite::SqliteConnectionManager;
    let manager = match path {
        Some(path) => SqliteConnectionManager::file(path),
        None => SqliteConnectionManager::memory(),
    };
    let customizer = Box::new(ConnCustomizer { synchronous_level });

    /*
     * We want
     * - num_read_threads connections for standard read limit
     * - num_read_threads for use in long running read transactions, to allow the normal pool to continue to be used
     * - 1 connection for writing
     */
    let max_cons = num_read_threads() * 2 + 1;

    r2d2::Pool::builder()
        // Only up to 20 connections at a time
        .max_size(max_cons as u32)
        // Never maintain idle connections
        .min_idle(Some(0))
        // Close connections after 30-60 seconds of idle time
        .idle_timeout(Some(Duration::from_secs(30)))
        .connection_timeout(Duration::from_millis(
            CONNECTION_TIMEOUT_MS.load(Ordering::Acquire),
        ))
        .thread_pool(R2D2_THREADPOOL.clone())
        .connection_customizer(customizer)
        .build(manager)
        .unwrap()
}

#[derive(Debug)]
struct ConnCustomizer {
    synchronous_level: DbSyncLevel,
}

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for ConnCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        initialize_connection(conn, self.synchronous_level)?;
        Ok(())
    }
}

pub(super) fn initialize_connection(
    conn: &mut Connection,
    synchronous_level: DbSyncLevel,
) -> Result<()> {
    // Tell SQLite to wait this long during write contention.
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

    #[cfg(feature = "sqlite-encrypted")]
    {
        use std::io::Write;
        let key = get_encryption_key_shim();
        let mut hex = *br#"0000000000000000000000000000000000000000000000000000000000000000"#;
        let mut c = std::io::Cursor::new(&mut hex[..]);
        for b in &key {
            write!(c, "{:02X}", b)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }
        let _keyval = std::str::from_utf8(&hex).unwrap();
        // conn.pragma_update(None, "key", &keyval)?;
        conn.pragma_update(None, "key", FAKE_KEY)?;
    }

    // this is recommended to always be off:
    // https://sqlite.org/pragma.html#pragma_trusted_schema
    conn.pragma_update(None, "trusted_schema", false)?;

    // enable foreign key support
    conn.pragma_update(None, "foreign_keys", "ON".to_string())?;

    match synchronous_level {
        DbSyncLevel::Full => conn.pragma_update(None, "synchronous", "2".to_string())?,
        DbSyncLevel::Normal => conn.pragma_update(None, "synchronous", "1".to_string())?,
        DbSyncLevel::Off => conn.pragma_update(None, "synchronous", "0".to_string())?,
    }

    add_custom_functions(conn)?;

    Ok(())
}

pub fn num_read_threads() -> usize {
    let num_cpus = num_cpus::get();
    let num_threads = num_cpus.checked_div(2).unwrap_or(0);
    std::cmp::max(num_threads, 4)
}

#[cfg(feature = "sqlite-encrypted")]
/// Simulate getting an encryption key from Lair.
fn get_encryption_key_shim() -> [u8; 32] {
    [
        26, 111, 7, 31, 52, 204, 156, 103, 203, 171, 156, 89, 98, 51, 158, 143, 57, 134, 93, 56,
        199, 225, 53, 141, 39, 77, 145, 130, 136, 108, 96, 201,
    ]
}

#[cfg(feature = "test_utils")]
pub fn set_connection_timeout(timeout_ms: u64) {
    CONNECTION_TIMEOUT_MS.store(timeout_ms, Ordering::Relaxed);
}

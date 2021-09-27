use crate::prelude::*;
use chashmap::CHashMap;
use holochain_serialized_bytes::prelude::*;
use once_cell::sync::Lazy;
use rusqlite::*;
use scheduled_thread_pool::ScheduledThreadPool;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

mod singleton_conn;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) static DATABASE_HANDLES: Lazy<CHashMap<PathBuf, DbWrite>> = Lazy::new(|| {
    // This is just a convenient place that we know gets initialized
    // both in the final binary holochain && in all relevant tests
    //
    // Holochain (and most binaries) are left in invalid states
    // if a thread panic!s - switch to failing fast in that case.
    //
    // We tried putting `panic = "abort"` in the Cargo.toml,
    // but somehow that breaks the wasmer / test_utils integration.

    let orig_handler = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // print the panic message
        eprintln!("FATAL PANIC {:#?}", panic_info);
        // invoke the original handler
        orig_handler(panic_info);
        // // Abort the process
        // // TODO - we need a better solution than this, but if there is
        // // no better solution, we can uncomment the following line:
        // std::process::abort();
    }));

    CHashMap::new()
});

static R2D2_THREADPOOL: Lazy<Arc<ScheduledThreadPool>> = Lazy::new(|| {
    let t = ScheduledThreadPool::new(1);
    Arc::new(t)
});

pub type ConnectionPool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
pub type PConnInner = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub(crate) fn new_connection_pool(
    path: &Path,
    kind: DbKind,
    synchronous_level: DbSyncLevel,
) -> ConnectionPool {
    use r2d2_sqlite::SqliteConnectionManager;
    let manager = SqliteConnectionManager::file(path);
    let customizer = Box::new(ConnCustomizer {
        kind,
        synchronous_level,
    });
    // We need the same amount of connections as reader threads plus one for the writer thread.
    let max_cons = num_read_threads() + 1;
    r2d2::Pool::builder()
        // Only up to 20 connections at a time
        .max_size(max_cons as u32)
        // Never maintain idle connections
        .min_idle(Some(0))
        // Close connections after 30-60 seconds of idle time
        .idle_timeout(Some(Duration::from_secs(30)))
        .thread_pool(R2D2_THREADPOOL.clone())
        .connection_customizer(customizer)
        .build(manager)
        .unwrap()
}

#[derive(Debug)]
struct ConnCustomizer {
    kind: DbKind,
    synchronous_level: DbSyncLevel,
}

/// The sqlite synchronous level.
/// Corresponds to the `PRAGMA synchronous` pragma.
/// See [sqlite documentation](https://www.sqlite.org/pragma.html#pragma_synchronous).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub enum DbSyncLevel {
    /// Use xSync for all writes. Not needed for WAL mode.
    Full,
    /// Sync at critical moments. Default.
    Normal,
    /// Syncing is left to the operating system and power loss could result in corrupted database.
    Off,
}

impl Default for DbSyncLevel {
    fn default() -> Self {
        DbSyncLevel::Normal
    }
}

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for ConnCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        initialize_connection(conn, &self.kind, self.synchronous_level, true)?;
        Ok(())
    }
}

fn initialize_connection(
    conn: &mut Connection,
    _kind: &DbKind,
    synchronous_level: DbSyncLevel,
    _is_first: bool,
) -> rusqlite::Result<()> {
    // tell SQLite to wait this long during write contention
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

    #[cfg(feature = "db-encryption")]
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
        const FAKE_KEY: &str = "x'98483C6EB40B6C31A448C22A66DED3B5E5E8D5119CAC8327B655C8B5C483648101010101010101010101010101010101'";
        // conn.pragma_update(None, "key", &keyval)?;
        conn.pragma_update(None, "key", &FAKE_KEY)?;
    }

    // this is recommended to always be off:
    // https://sqlite.org/pragma.html#pragma_trusted_schema
    conn.pragma_update(None, "trusted_schema", &false)?;

    // enable foreign key support
    conn.pragma_update(None, "foreign_keys", &"ON".to_string())?;

    match synchronous_level {
        DbSyncLevel::Full => conn.pragma_update(None, "synchronous", &"2".to_string())?,
        DbSyncLevel::Normal => conn.pragma_update(None, "synchronous", &"1".to_string())?,
        DbSyncLevel::Off => conn.pragma_update(None, "synchronous", &"0".to_string())?,
    }

    Ok(())
}

#[cfg(feature = "db-encryption")]
/// Simulate getting an encryption key from Lair.
fn get_encryption_key_shim() -> [u8; 32] {
    [
        26, 111, 7, 31, 52, 204, 156, 103, 203, 171, 156, 89, 98, 51, 158, 143, 57, 134, 93, 56,
        199, 225, 53, 141, 39, 77, 145, 130, 136, 108, 96, 201,
    ]
}

/// Singleton Connection.
#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct PConn {
    #[shrinkwrap(main_field)]
    inner: PConnInner,
    _kind: DbKind,
}

impl PConn {
    pub(crate) fn new(inner: PConnInner, _kind: DbKind) -> Self {
        Self { inner, _kind }
    }
}

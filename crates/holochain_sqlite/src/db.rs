//! Functions dealing with obtaining and referencing singleton LMDB environments

use crate::{prelude::*, swansong::SwanSong};
use derive_more::Into;
use holochain_keystore::KeystoreSender;
use holochain_zome_types::cell::CellId;
use lazy_static::lazy_static;
use parking_lot::{Mutex, MutexGuard, RwLock};
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

lazy_static! {

    static ref CONNECTIONS: RwLock<HashMap<PathBuf, Conn>> = {
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

        RwLock::new(HashMap::new())
    };
}

/// A read-only version of [DbWrite].
/// This environment can only generate read-only transactions, never read-write.
#[derive(Clone)]
pub struct DbRead {
    kind: DbKind,
    path: PathBuf,
    keystore: KeystoreSender,
}

impl DbRead {
    #[deprecated = "rename to `conn`"]
    pub fn guard(&self) -> Conn {
        self.connection_naive().expect("TODO: Can't fail")
    }

    #[deprecated = "remove this identity function"]
    pub fn inner(&self) -> Self {
        self.clone()
    }

    /// Accessor for the [DbKind] of the DbWrite
    pub fn kind(&self) -> &DbKind {
        &self.kind
    }

    /// Request access to this conductor's keystore
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// The environment's path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    #[deprecated = "TODO: use `connection`"]
    fn connection_naive(&self) -> DatabaseResult<Conn> {
        Ok(Conn::open(&self.path, &self.kind)?)
    }

    // fn connection(&self) -> DatabaseResult<Conn> {
    //     CONNECTIONS.with(|conns| {

    //         conns.borrow_mut().get_mut(k)
    //     });
    // }
}

impl GetTable for DbRead {}
impl GetTable for DbWrite {}

/// The canonical representation of a (singleton) LMDB environment.
/// The wrapper contains methods for managing transactions
/// and database connections,
#[derive(Clone, Shrinkwrap, Into, derive_more::From)]
pub struct DbWrite(DbRead);

impl DbWrite {
    /// Create an environment,
    pub fn new(
        path_prefix: &Path,
        kind: DbKind,
        keystore: KeystoreSender,
    ) -> DatabaseResult<DbWrite> {
        if !path_prefix.is_dir() {
            std::fs::create_dir(path_prefix.clone())
                .map_err(|_e| DatabaseError::EnvironmentMissing(path_prefix.to_owned()))?;
        }
        let path = path_prefix.join(kind.filename());
        let env: DbWrite = DbWrite(DbRead {
            kind,
            keystore,
            path,
        });
        Ok(env)
    }

    /// Create a Cell environment (slight shorthand)
    pub fn new_cell(
        path_prefix: &Path,
        cell_id: CellId,
        keystore: KeystoreSender,
    ) -> DatabaseResult<Self> {
        Self::new(path_prefix, DbKind::Cell(cell_id), keystore)
    }

    #[deprecated = "remove this identity function"]
    pub fn guard(&self) -> Conn {
        self.0.guard()
    }

    /// Remove the db and directory
    pub async fn remove(self) -> DatabaseResult<()> {
        std::fs::remove_dir_all(&self.0.path)?;
        Ok(())
    }
}

/// Wrapper around Connection.
/// Needed to allow borrowing transactions in the same fashion as our LMDB
/// lifetime model
// #[derive(Shrinkwrap)]
#[derive(Clone)]
pub struct Conn {
    inner: Arc<Mutex<Connection>>,
    kind: DbKind,
}

fn initialize_connection(
    mut conn: Connection,
    kind: &DbKind,
    is_first: bool,
) -> DatabaseResult<Connection> {
    // tell SQLite to wait this long during write contention
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)?;

    let key = get_encryption_key_shim();
    let mut cmd =
        *br#"PRAGMA key = "x'0000000000000000000000000000000000000000000000000000000000000000'";"#;
    {
        use std::io::Write;
        let mut c = std::io::Cursor::new(&mut cmd[16..80]);
        for b in &key {
            write!(c, "{:02X}", b)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }
    }
    conn.execute(std::str::from_utf8(&cmd).unwrap(), NO_PARAMS)?;

    // set to faster write-ahead-log mode
    conn.pragma_update(None, "journal_mode", &"WAL".to_string())?;

    if is_first {
        initialize_database(&mut conn, &kind)?;
    }

    Ok(conn)
}

impl Conn {
    /// Create a new connection with decryption key set
    pub fn open(path: &Path, kind: &DbKind) -> DatabaseResult<Self> {
        let mut map = CONNECTIONS.write();
        let conn = match map.entry(path.to_owned()) {
            Entry::Vacant(e) => {
                let conn = Self::new(
                    initialize_connection(Connection::open(path)?, kind, true)?,
                    kind.clone(),
                );
                e.insert(conn).clone()
            }
            Entry::Occupied(e) => e.get().clone(),
        };

        Ok(conn)
    }

    fn new(inner: Connection, kind: DbKind) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            kind,
        }
    }

    pub fn inner(&mut self) -> SwanSong<MutexGuard<Connection>> {
        let kind = self.kind.clone();
        tracing::trace!("lock attempt {}", &kind);
        let guard = self
            .inner
            .try_lock_for(std::time::Duration::from_secs(30))
            .expect(&format!("Couldn't unlock connection. Kind: {}", &kind));
        tracing::trace!("lock success {}", &kind);
        SwanSong::new(guard, move |_| {
            tracing::trace!("lock drop {}", &kind);
        })
    }

    #[cfg(feature = "test_utils")]
    pub fn open_single(&mut self, name: &str) -> Result<SingleTable, DatabaseError> {
        crate::table::initialize_table_single(
            &mut self.inner(),
            name.to_string(),
            name.to_string(),
        )?;
        Ok(Table {
            name: TableName::TestSingle(name.to_string()),
        })
    }

    #[cfg(feature = "test_utils")]
    pub fn open_integer(&mut self, name: &str) -> Result<IntegerTable, DatabaseError> {
        self.open_single(name)
    }

    #[cfg(feature = "test_utils")]
    pub fn open_multi(&mut self, name: &str) -> Result<MultiTable, DatabaseError> {
        crate::table::initialize_table_multi(
            &mut self.inner(),
            name.to_string(),
            name.to_string(),
        )?;
        Ok(Table {
            name: TableName::TestMulti(name.to_string()),
        })
    }
}

/// Simulate getting an encryption key from Lair.
fn get_encryption_key_shim() -> [u8; 32] {
    [
        26, 111, 7, 31, 52, 204, 156, 103, 203, 171, 156, 89, 98, 51, 158, 143, 57, 134, 93, 56,
        199, 225, 53, 141, 39, 77, 145, 130, 136, 108, 96, 201,
    ]
}

#[deprecated = "Shim for `Rkv`, just because we have methods that call these"]
pub struct ConnInner;

impl ConnInner {}

/// The various types of LMDB environment, used to specify the list of databases to initialize
#[derive(Clone, Debug, derive_more::Display)]
pub enum DbKind {
    /// Specifies the environment used by each Cell
    Cell(CellId),
    /// Specifies the environment used by a Conductor
    Conductor,
    /// Specifies the environment used to save wasm
    Wasm,
    /// State of the p2p network
    P2p,
}

impl DbKind {
    /// Constuct a partial Path based on the kind
    fn filename(&self) -> PathBuf {
        let mut path = match self {
            DbKind::Cell(cell_id) => PathBuf::from(cell_id.to_string()),
            DbKind::Conductor => PathBuf::from("conductor"),
            DbKind::Wasm => PathBuf::from("wasm"),
            DbKind::P2p => PathBuf::from("p2p"),
        };
        path.set_extension("sqlite3");
        path
    }
}

/// Implementors are able to create a new read-only LMDB transaction
pub trait ReadManager<'e> {
    /// Run a closure, passing in a new read-only transaction
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Reader) -> Result<R, E>;

    #[cfg(feature = "test_utils")]
    /// Same as with_reader, but with no Results: everything gets unwrapped
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Reader) -> R;
}

/// Implementors are able to create a new read-write LMDB transaction
pub trait WriteManager<'e> {
    /// Run a closure, passing in a mutable reference to a read-write
    /// transaction, and commit the transaction after the closure has run.
    /// If there is a LMDB error, recover from it and re-run the closure.
    // FIXME: B-01566: implement write failure detection
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Writer) -> Result<R, E>;

    // /// Get a raw read-write transaction for this environment.
    // /// It is preferable to use WriterManager::with_commit for database writes,
    // /// which can properly recover from and manage write failures
    // fn writer_unmanaged(&'e mut self) -> DatabaseResult<Writer<'e>>;
}

impl<'e> ReadManager<'e> for Conn {
    fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(Reader) -> Result<R, E>,
    {
        let mut g = self.inner();
        let txn = g.transaction().map_err(DatabaseError::from)?;
        let reader = Reader::from(txn);
        f(reader)
    }

    #[cfg(feature = "test_utils")]
    fn with_reader_test<R, F>(&'e mut self, f: F) -> R
    where
        F: 'e + FnOnce(Reader) -> R,
    {
        self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
    }
}

// impl<'e> ReadManager<'e> for DbWrite {
//     fn reader(&'e mut self) -> DatabaseResult<Reader<'e>> {
//         let mut conn = self.connection_naive()?;
//         let txn = conn.transaction()?;
//         let mut reader = Reader::from(txn);
//         Ok(reader)
//     }

//     fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
//     where
//         E: From<DatabaseError>,
//         F: 'e + FnOnce(Reader) -> Result<R, E>,
//     {
//         f(self.reader()?)
//     }
// }

impl<'e> WriteManager<'e> for Conn {
    fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
    where
        E: From<DatabaseError>,
        F: 'e + FnOnce(&mut Writer) -> Result<R, E>,
    {
        let mut b = self.inner();
        let txn = b.transaction().map_err(DatabaseError::from)?;
        let mut writer = Writer::from(txn);
        let result = f(&mut writer)?;
        writer.commit().map_err(DatabaseError::from)?;
        Ok(result)
    }

    // fn writer_unmanaged(&'e mut self) -> DatabaseResult<Writer<'e>> {
    //     let mut b = self.conn.borrow_mut();
    //     let txn = b.transaction()?;
    //     let writer = Writer::from(txn);
    //     Ok(writer)
    // }
}

// impl<'e> WriteManager<'e> for DbWrite {
//     fn with_commit<E, R, F: Send>(&'e self, f: F) -> Result<R, E>
//     where
//         E: From<DatabaseError>,
//         F: 'e + FnOnce(&mut Writer) -> Result<R, E>,
//     {
//         Conn::with_commit(&self.connection()?, f)
//     }
// }

//! This module is just wrappers around the rkv transaction representations.
//! They are necessary/useful for a few reasons:
//! - Reader is not marked Send + Sync in rkv, but we must mark it such to make
//!     use of the threadsafe read-only transactions provided by the MDB_NOTLS flag
//! - We can upgrade some error types from rkv::StoreError, which does not implement
//!     std::error::Error, into error types that do

use crate::error::DatabaseError;
use chrono::offset::Local;
use chrono::DateTime;
use derive_more::From;
use rkv::Database;
use rkv::RoCursor;
use rkv::Value;
use shrinkwraprs::Shrinkwrap;

/// Just a trait alias for rkv::Readable
/// It's important because it lets us use either a Reader or a Writer
/// for read-only operations
pub trait Readable: rkv::Readable {}
impl<T: rkv::Readable> Readable for T {}

struct ReaderSpanInfo {
    // Using a chrono timestamp here because we need duration operations
    start_time: DateTime<Local>,
}

impl ReaderSpanInfo {
    pub fn new() -> Self {
        Self {
            start_time: Local::now(),
        }
    }
}

impl Drop for ReaderSpanInfo {
    fn drop(&mut self) {
        let ms = Local::now()
            .signed_duration_since(self.start_time)
            .num_milliseconds();
        if ms >= 100 {
            tracing::warn!("long-lived reader: {} ms", ms);
        }
    }
}

/// Wrapper around `rkv::Reader`, so it can be marked as threadsafe
#[derive(Shrinkwrap)]
pub struct Reader<'env>(#[shrinkwrap(main_field)] rkv::Reader<'env>, ReaderSpanInfo);

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Send for Reader<'env> {}

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Sync for Reader<'env> {}

impl<'env> rkv::Readable for Reader<'env> {
    fn get<K: AsRef<[u8]>>(&self, db: Database, k: &K) -> Result<Option<Value>, StoreError> {
        self.0.get(db, k)
    }

    fn open_ro_cursor(&self, db: Database) -> Result<RoCursor, StoreError> {
        self.0.open_ro_cursor(db)
    }
}

impl<'env> From<rkv::Reader<'env>> for Reader<'env> {
    fn from(r: rkv::Reader<'env>) -> Self {
        Self(r, ReaderSpanInfo::new())
    }
}

/// Wrapper around `rkv::Writer`, which lifts some of the return values to types recognized by this crate,
/// rather than the rkv-specific values
#[derive(From, Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct Writer<'env>(rkv::Writer<'env>);

impl<'env> rkv::Readable for Writer<'env> {
    fn get<K: AsRef<[u8]>>(&self, db: Database, k: &K) -> Result<Option<Value>, StoreError> {
        self.0.get(db, k)
    }

    fn open_ro_cursor(&self, db: Database) -> Result<RoCursor, StoreError> {
        self.0.open_ro_cursor(db)
    }
}

impl<'env> Writer<'env> {
    /// This override exists solely to raise the Error from the rkv::StoreError,
    /// which does not implement std::error::Error, into a DatabaseError, which does.
    pub fn commit(self) -> Result<(), DatabaseError> {
        self.0.commit().map_err(DatabaseError::from)
    }
}

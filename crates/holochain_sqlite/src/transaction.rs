//! This module is just wrappers around the rkv transaction representations.
//! They are necessary/useful for a few reasons:
//! - Reader is not marked Send + Sync in rkv, but we must mark it such to make
//!     use of the threadsafe read-only transactions provided by the MDB_NOTLS flag
//! - We can upgrade some error types from rkv::StoreError, which does not implement
//!     std::error::Error, into error types that do

use crate::{db::Table, error::DatabaseError};
use chrono::offset::Local;
use chrono::DateTime;
use derive_more::From;
use rkv::StoreError;
use rkv::Value;
use rusqlite::*;
use shrinkwraprs::Shrinkwrap;

#[deprecated = "no need for read/write distinction with SQLite"]
pub trait Readable {
    fn get<K: AsRef<[u8]>>(&self, db: Table, k: &K) -> Result<Option<Value>, StoreError>;
}

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
pub struct Reader<'env>(#[shrinkwrap(main_field)] Transaction<'env>, ReaderSpanInfo);

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Send for Reader<'env> {}

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Sync for Reader<'env> {}

impl<'env> Readable for Reader<'env> {
    fn get<K: AsRef<[u8]>>(&self, db: Table, k: &K) -> Result<Option<Value>, StoreError> {
        todo!("do get on the table with sqlite")
    }
}

impl<'env> From<Transaction<'env>> for Reader<'env> {
    fn from(r: Transaction<'env>) -> Self {
        Self(r, ReaderSpanInfo::new())
    }
}

/// Wrapper around `rkv::Writer`, which lifts some of the return values to types recognized by this crate,
/// rather than the rkv-specific values
#[derive(From, Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct Writer<'env>(Transaction<'env>);

impl<'env> Readable for Writer<'env> {
    fn get<K: AsRef<[u8]>>(&self, db: Table, k: &K) -> Result<Option<Value>, StoreError> {
        todo!("do get on the table with sqlite")
    }
}

impl<'env> Writer<'env> {
    /// This override exists solely to raise the Error from the rkv::StoreError,
    /// which does not implement std::error::Error, into a DatabaseError, which does.
    pub fn commit(self) -> Result<(), DatabaseError> {
        todo!("sqlite")
    }
}

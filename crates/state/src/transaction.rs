//! This module is just wrappers around the rkv transaction representations.
//! They are necessary/useful for a few reasons:
//! - Reader is not marked Send + Sync in rkv, but we must mark it such to make
//!     use of the threadsafe read-only transactions provided by the MDB_NOTLS flag
//! - We can upgrade some error types from rkv::StoreError, which does not implement
//!     std::error::Error, into error types that do

use crate::{env::EnvReadRef, error::DatabaseError};
use rkv::{Database, RoCursor, StoreError, Value};
use shrinkwraprs::Shrinkwrap;
use derive_more::{From};

/// Just a trait alias for rkv::Readable
/// It's important because it lets us use either a Reader or a Writer
/// for read-only operations
pub trait Readable: rkv::Readable {}
impl<T: rkv::Readable> Readable for T {}

#[derive(From, Shrinkwrap)]
pub struct ThreadsafeRkvReader<'env>(rkv::Reader<'env>);

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Send for ThreadsafeRkvReader<'env> {}

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Sync for ThreadsafeRkvReader<'env> {}


#[derive(From, Shrinkwrap)]
pub struct Reader<'env>(ThreadsafeRkvReader<'env>);

impl<'env> rkv::Readable for Reader<'env> {
    fn get<K: AsRef<[u8]>>(&self, db: Database, k: &K) -> Result<Option<Value>, StoreError> {
        self.0.get(db, k)
    }

    fn open_ro_cursor(&self, db: Database) -> Result<RoCursor, StoreError> {
        self.0.open_ro_cursor(db)
    }
}

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

#![feature(backtrace)]
use error::WorkspaceResult;
use shrinkwraprs::Shrinkwrap;
use lmdb::{RoCursor, Database};
use rkv::{Value, StoreError, Rkv};
use std::sync::{RwLock, Arc, RwLockReadGuard};

pub mod buffer;
pub mod db;
pub mod env;
pub mod error;

// NB: would be nice to put this under cfg(test), but then it's not visible from other crates,
// since cfg(test) only applies to the crate in which you run tests
pub mod test_utils;

// trait alias
pub trait Readable: rkv::Readable {}
impl<T: rkv::Readable> Readable for T {}

#[derive(Shrinkwrap)]
pub struct Reader<'env>(rkv::Reader<'env>);

impl<'env> rkv::Readable for Reader<'env> {
    fn get<K: AsRef<[u8]>>(&self, db: Database, k: &K) -> Result<Option<Value>, StoreError> {
        self.0.get(db, k)
    }

    fn open_ro_cursor(&self, db: Database) -> Result<RoCursor, StoreError> {
        self.0.open_ro_cursor(db)
    }
}

impl<'env> From<rkv::Reader<'env>> for Reader<'env> {
    fn from(r: rkv::Reader<'env>) -> Reader {
        Reader(r)
    }
}

pub type Writer<'env> = rkv::Writer<'env>;
pub type SingleStore = rkv::SingleStore;
pub type IntegerStore = rkv::IntegerStore<u32>;
pub type MultiStore = rkv::MultiStore;

#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Send for Reader<'env> {}

#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Sync for Reader<'env> {}

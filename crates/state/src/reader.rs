use lmdb::{Database, RoCursor};
use rkv::{StoreError, Value};
use shrinkwraprs::Shrinkwrap;

/// Just a trait alias for rkv::Readable
/// It's important because it lets us use either a Reader or a Writer
/// for read-only operations
pub trait Readable: rkv::Readable {}
impl<T: rkv::Readable> Readable for T {}

#[derive(Shrinkwrap)]
pub struct Reader<'env>(rkv::Reader<'env>);

impl<'env> Reader<'env> {
    pub(crate) fn new(inner: rkv::Reader<'env>) -> Self {
        Self(inner)
    }
}

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

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Send for Reader<'env> {}

/// If MDB_NOTLS env flag is set, then read-only transactions are threadsafe
/// and we can mark them as such
#[cfg(feature = "lmdb_no_tls")]
unsafe impl<'env> Sync for Reader<'env> {}

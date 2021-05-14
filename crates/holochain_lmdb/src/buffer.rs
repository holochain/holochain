#![allow(missing_docs)]
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::transaction::Writer;

mod cas;
pub mod iter;
mod kv;
mod kvv;

pub use cas::CasBufFreshAsync;
pub use cas::CasBufFreshSync;
pub use kv::KvBufFresh;
pub use kv::KvBufUsed;
pub use kv::KvIntBufFresh;
pub use kv::KvIntBufUsed;
pub use kv::KvIntStore;
pub use kv::KvStore;
pub use kv::KvStoreT;
pub use kvv::KvvBufUsed;

// Empty keys break lmdb
pub(super) fn check_empty_key<K: AsRef<[u8]>>(k: &K) -> DatabaseResult<()> {
    if k.as_ref().is_empty() {
        Err(DatabaseError::EmptyKey)
    } else {
        Ok(())
    }
}

/// General trait for transactional stores, exposing only the method which
/// adds changes to the write transaction. This generalization is not really used,
/// but could be used in Workspaces i.e. iterating over a Vec<dyn BufferedStore>
/// is all that needs to happen to commit the workspace changes
pub trait BufferedStore: Sized {
    /// The error type for `flush_to_txn` errors
    type Error: std::error::Error;

    /// Flush the scratch space to the read-write transaction, staging the changes
    /// for an actual database update
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> Result<(), Self::Error>;

    fn flush_to_txn(mut self, writer: &mut Writer) -> Result<(), Self::Error> {
        self.flush_to_txn_ref(writer)
    }

    /// Specifies whether there are actually changes to flush. If not, the
    /// flush_to_txn method may decide to do nothing.
    fn is_clean(&self) -> bool {
        false
    }
}

#[macro_export]
/// Macro to generate a fresh reader from an EnvironmentRead with less boilerplate
macro_rules! fresh_reader {
    ($env: expr, $f: expr) => {{
        let g = $env.guard();
        let r = $crate::env::ReadManager::reader(&g)?;
        $f(r)
    }};
}

#[macro_export]
/// Macro to generate a fresh reader from an EnvironmentRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! fresh_reader_test {
    ($env: expr, $f: expr) => {{
        let g = $env.guard();
        let r = $crate::env::ReadManager::reader(&g).unwrap();
        $f(r)
    }};
}

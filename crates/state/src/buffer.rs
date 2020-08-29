#![allow(missing_docs)]
use crate::{
    error::{DatabaseError, DatabaseResult},
    transaction::Writer,
};

mod cas;
pub mod iter;
mod kv;
mod kvv;

pub use cas::CasBufFreshAsync;
pub use kv::{KvBufFresh, KvBufUsed, KvIntBufFresh, KvIntBufUsed, KvIntStore, KvStore, KvStoreT};
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
pub trait BufferedStore {
    /// The error type for `flush_to_txn` errors
    type Error: std::error::Error;

    /// Flush the scratch space to the read-write transaction, staging the changes
    /// for an actual database update
    fn flush_to_txn(self, writer: &mut Writer) -> Result<(), Self::Error>;

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
        let g = $env.guard().await;
        let r = g.reader()?;
        $f(r)
    }};
}

#[macro_export]
/// Macro to generate a fresh reader from an EnvironmentRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! fresh_reader_test {
    ($env: expr, $f: expr) => {{
        let g = $env.guard().await;
        let r = g.reader().unwrap();
        $f(r)
    }};
}

#[macro_export]
/// Use this variant of `fresh_reader` when the $f closure is async
macro_rules! fresh_reader_async {
    ($env: expr, $f: expr) => {{
        let env = $env.clone();
        let g = env.guard().await;
        let r = g.reader()?;
        let val = $f(r).await;
        val
    }};
}

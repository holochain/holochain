//! Functions dealing with obtaining and referencing singleton databases

mod access;
mod conn;
mod databases;
mod guard;
mod key;
mod kind;
mod metrics;
mod pool;

#[cfg(all(test, not(loom)))]
mod tests;

#[cfg(feature = "test_utils")]
pub use access::set_acquire_timeout;
pub use access::{DbRead, DbWrite, ReadAccess, Txn};
pub use guard::PTxnGuard;
pub use key::DbKey;
pub use kind::{
    DbKind, DbKindAuthored, DbKindCache, DbKindConductor, DbKindDht, DbKindOp, DbKindPeerMetaStore,
    DbKindT, DbKindWasm,
};
#[cfg(feature = "test_utils")]
pub use pool::{num_read_threads, set_connection_timeout};
pub use pool::{DbSyncLevel, DbSyncStrategy, PoolConfig};

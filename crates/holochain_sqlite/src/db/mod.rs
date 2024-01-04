//! Functions dealing with obtaining and referencing singleton databases

mod access;
mod conn;
mod databases;
mod guard;
mod kind;
mod metrics;
mod pool;

#[cfg(all(test, not(loom)))]
mod tests;

pub use access::{DbRead, DbWrite, ReadAccess};
pub use guard::PTxnGuard;
pub use kind::{
    DbKind, DbKindAuthored, DbKindCache, DbKindConductor, DbKindDht, DbKindOp, DbKindP2pAgents,
    DbKindP2pMetrics, DbKindT, DbKindWasm,
};
pub use pool::{DbSyncLevel, DbSyncStrategy};

#[cfg(feature = "test_utils")]
pub use access::set_acquire_timeout;
#[cfg(feature = "test_utils")]
pub use pool::{num_read_threads, set_connection_timeout};

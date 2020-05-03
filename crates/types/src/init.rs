//! the _host_ types used to track the status/result of the initialization process
//! c.f. _guest_ types that co-ordinate the init callbacks across the wasm boudary in zome_types

use crate::nucleus::ZomeName;
use holo_hash::EntryHash;

/// the aggregate result of _all_ init callbacks
pub enum InitDnaResult {
    /// all init callbacks passed
    Pass,
    /// some init failed
    /// ZomeName is the first zome that failed to init
    /// String is a human-readable error string giving the reason for failure
    Fail(ZomeName, String),
    /// no init failed but some zome has unresolved dependencies
    /// ZomeName is the first zome that has unresolved dependencies
    /// Vec<EntryHash> is the list of all missing dependency addresses
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
}

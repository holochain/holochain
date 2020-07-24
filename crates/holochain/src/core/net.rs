//! Placeholder for networking related types. May be deleted.

use holo_hash::EntryHash;

/// Placeholder for a resource which can be passed into Workflows,
/// granting access to the networking subsystem
pub trait NetRequester {
    /// Asks the networking subsystem to fetch an entry
    fn fetch_entry(address: EntryHash);
}

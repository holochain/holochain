//! Defines a Record, the basic unit of Holochain data.

// Legacy record-support types (`RecordEntry`, `SignedHashed`, etc.) that v2
// reuses unchanged.
pub use holochain_integrity_types::record::*;

/// The canonical chain record: a signed, hashed v2 action plus its entry.
pub use crate::dht_v2::Record;
/// A v2 action with its signature (no hash).
pub use crate::dht_v2::SignedAction;
/// A v2 action that is both content-addressed and signed.
pub use crate::dht_v2::SignedActionHashed;

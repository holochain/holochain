//! Defines a Record, the basic unit of Holochain data.

// Record-support types (`RecordEntry`, `SignedHashed`, etc.).
pub use holochain_integrity_types::record::*;

/// The canonical chain record: a signed, hashed action plus its entry.
pub use crate::dht_v2::Record;
/// An action with its signature (no hash).
pub use crate::dht_v2::SignedAction;
/// An action that is both content-addressed and signed.
pub use crate::dht_v2::SignedActionHashed;

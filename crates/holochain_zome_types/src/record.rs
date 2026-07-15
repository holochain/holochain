//! Defines a Record, the basic unit of Holochain data.

// Record-support types (`RecordEntry`, `SignedHashed`, etc.).
pub use holochain_integrity_types::record::*;

/// An action with its signature (no hash).
pub use crate::action::SignedAction;
/// An action that is both content-addressed and signed.
pub use crate::action::SignedActionHashed;

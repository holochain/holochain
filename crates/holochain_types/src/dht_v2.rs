//! Redesigned DHT state-model op types (transitional — see `docs/design/state_model.md`).

pub use holochain_zome_types::dht_v2::*;

use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::Entry;

/// How an entry is represented inside a `ChainOp`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub enum OpEntry {
    /// The entry is included with the op.
    Present(Entry),
    /// The action references a private entry, which is not included.
    Hidden,
    /// The action type doesn't have an associated entry.
    ActionOnly,
}

/// Chain-level DHT ops. Each variant targets a specific authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub enum ChainOp {
    /// Store an action record at the action's hash authority.
    CreateRecord(SignedAction, OpEntry),
    /// Store an entry (+ its create action) at the entry's hash authority.
    CreateEntry(SignedAction, OpEntry),
    /// Register activity on an agent's source chain at the agent authority.
    AgentActivity(SignedAction),
    /// Register an updated entry at the entry authority of the new entry.
    UpdateEntry(SignedAction, OpEntry),
    /// Register an updated record at the original action's authority.
    UpdateRecord(SignedAction, OpEntry),
    /// Register a delete-entry at the entry authority.
    DeleteEntry(SignedAction),
    /// Register a delete-record at the action authority.
    DeleteRecord(SignedAction),
    /// Register a link creation at the link base authority.
    CreateLink(SignedAction),
    /// Register a link deletion at the link base authority.
    DeleteLink(SignedAction),
}

/// A warrant op. Thin wrapper so `DhtOp` can carry both chain and warrant ops
/// as a single sum type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct WarrantOp(pub SignedWarrant);

/// Top-level DHT op.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub enum DhtOp {
    /// A chain-level op (record/entry/activity/link).
    ChainOp(Box<ChainOp>),
    /// A warrant op.
    WarrantOp(Box<WarrantOp>),
}

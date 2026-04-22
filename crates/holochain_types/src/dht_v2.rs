//! Redesigned DHT state-model op types (transitional — see `docs/design/state_model.md`).

pub use holochain_zome_types::dht_v2::*;

use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash, EntryHash, HasHash, HoloHashed};
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::op::ChainOpType;
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

/// Internal representation of a `ChainOp` with all hashes pre-computed.
/// Used during the incoming-ops workflow so hashes aren't recomputed for
/// each database write.
#[derive(Clone, Debug)]
pub struct HashedChainOp {
    /// The hash of this op.
    pub op_hash: DhtOpHash,
    /// The signed action with its pre-computed hash.
    pub action: SignedActionHashed,
    /// The entry (if any) with its pre-computed hash.
    pub entry: Option<HoloHashed<Entry>>,
    /// The type discriminant of the op.
    pub op_type: ChainOpType,
    /// The DHT basis hash (where the op is stored).
    pub basis_hash: AnyDhtHash,
    /// The numeric storage center derived from `basis_hash`.
    pub storage_center_loc: u32,
}

impl HashedChainOp {
    /// Return the action hash of the wrapped signed action.
    pub fn action_hash(&self) -> &ActionHash {
        self.action.as_hash()
    }

    /// Return the entry hash if this op carries an entry.
    pub fn entry_hash(&self) -> Option<&EntryHash> {
        self.entry.as_ref().map(|e| e.as_hash())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_chain_op_accessors_compile() {
        fn _assert(h: &HashedChainOp) -> (&ActionHash, Option<&EntryHash>) {
            (h.action_hash(), h.entry_hash())
        }
    }
}

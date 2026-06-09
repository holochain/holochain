//! Redesigned DHT state-model op types (transitional — see `docs/design/state_model.md`).

pub use holochain_zome_types::dht_v2::*;

use holo_hash::{
    hash_type, ActionHash, AnyDhtHash, DhtOpHash, EntryHash, HasHash, HashableContent,
    HashableContentBytes, HoloHashed,
};
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

/// The canonical, content-derived form a [`ChainOp`] is hashed over to produce
/// its [`DhtOpHash`].
///
/// Signatures and entries are deliberately excluded: the signature is redundant
/// with the action (and would make the hash unstable across re-signings), and
/// the entry is already addressed by the action's `entry_hash`. The enum
/// discriminant distinguishes ops that share an action — a `Create` produces
/// both a `CreateRecord` and a `CreateEntry` op — so one action yields distinct
/// op hashes per op type.
///
/// Unlike the legacy `ChainOpUniqueForm`, every variant borrows `&Action`
/// directly, because the v2 `Action` is a single flat struct rather than a
/// variant-per-type enum.
#[allow(missing_docs)]
#[derive(serde::Serialize, Debug)]
pub enum ChainOpUniqueForm<'a> {
    CreateRecord(&'a Action),
    CreateEntry(&'a Action),
    AgentActivity(&'a Action),
    UpdateEntry(&'a Action),
    UpdateRecord(&'a Action),
    DeleteEntry(&'a Action),
    DeleteRecord(&'a Action),
    CreateLink(&'a Action),
    DeleteLink(&'a Action),
}

impl HashableContent for ChainOpUniqueForm<'_> {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            UnsafeBytes::from(
                holochain_serialized_bytes::encode(self)
                    .expect("Could not serialize HashableContent"),
            )
            .into(),
        )
    }
}

impl ChainOp {
    /// The content-derived [`DhtOpHash`] for this op.
    ///
    /// Excludes the signature and entry (see [`ChainOpUniqueForm`]) and carries
    /// no `weight`, so it is reproducible from the v2 op alone.
    pub fn to_hash(&self) -> DhtOpHash {
        DhtOpHash::with_data_sync(&self.to_unique_form())
    }

    fn to_unique_form(&self) -> ChainOpUniqueForm<'_> {
        match self {
            ChainOp::CreateRecord(sa, _) => ChainOpUniqueForm::CreateRecord(sa.data()),
            ChainOp::CreateEntry(sa, _) => ChainOpUniqueForm::CreateEntry(sa.data()),
            ChainOp::AgentActivity(sa) => ChainOpUniqueForm::AgentActivity(sa.data()),
            ChainOp::UpdateEntry(sa, _) => ChainOpUniqueForm::UpdateEntry(sa.data()),
            ChainOp::UpdateRecord(sa, _) => ChainOpUniqueForm::UpdateRecord(sa.data()),
            ChainOp::DeleteEntry(sa) => ChainOpUniqueForm::DeleteEntry(sa.data()),
            ChainOp::DeleteRecord(sa) => ChainOpUniqueForm::DeleteRecord(sa.data()),
            ChainOp::CreateLink(sa) => ChainOpUniqueForm::CreateLink(sa.data()),
            ChainOp::DeleteLink(sa) => ChainOpUniqueForm::DeleteLink(sa.data()),
        }
    }
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
mod op_hash_tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, EntryHash};
    use holochain_timestamp::Timestamp;
    use holochain_zome_types::prelude::{AppEntryDef, EntryType, EntryVisibility};
    use holochain_zome_types::signature::Signature;
    use holochain_zome_types::Entry;

    fn create_action() -> Action {
        Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: Timestamp::from_micros(1_000),
                action_seq: 4,
                prev_action: Some(ActionHash::from_raw_36(vec![2u8; 36])),
            },
            data: ActionData::Create(CreateData {
                entry_type: EntryType::App(AppEntryDef::new(
                    0.into(),
                    0.into(),
                    EntryVisibility::Public,
                )),
                entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
            }),
        }
    }

    fn signed(action: Action, sig: u8) -> SignedAction {
        SignedAction::new(action, Signature::from([sig; 64]))
    }

    #[test]
    fn op_hash_is_deterministic() {
        // Two independently-built ops with identical content hash equally.
        let a = ChainOp::CreateRecord(signed(create_action(), 7), OpEntry::ActionOnly);
        let b = ChainOp::CreateRecord(signed(create_action(), 7), OpEntry::ActionOnly);
        assert_eq!(a.to_hash(), b.to_hash());
    }

    #[test]
    fn op_hash_differs_by_op_type() {
        let sa = signed(create_action(), 7);
        let record = ChainOp::CreateRecord(sa.clone(), OpEntry::ActionOnly);
        let entry = ChainOp::CreateEntry(sa, OpEntry::ActionOnly);
        assert_ne!(record.to_hash(), entry.to_hash());
    }

    #[test]
    fn op_hash_ignores_signature_and_entry() {
        let action = create_action();
        let with_sig_7 = ChainOp::CreateRecord(signed(action.clone(), 7), OpEntry::ActionOnly);
        let with_sig_9_and_entry = ChainOp::CreateRecord(
            signed(action, 9),
            OpEntry::Present(Entry::Agent(AgentPubKey::from_raw_36(vec![5u8; 36]))),
        );
        assert_eq!(with_sig_7.to_hash(), with_sig_9_and_entry.to_hash());
    }

    #[test]
    fn op_hash_is_content_derived() {
        let base = ChainOp::CreateRecord(signed(create_action(), 7), OpEntry::ActionOnly);
        let mut changed_action = create_action();
        changed_action.header.action_seq = 99;
        let changed = ChainOp::CreateRecord(signed(changed_action, 7), OpEntry::ActionOnly);
        assert_ne!(base.to_hash(), changed.to_hash());
    }
}

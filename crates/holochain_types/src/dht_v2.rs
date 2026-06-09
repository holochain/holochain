//! Redesigned DHT state-model op types (transitional — see `docs/design/state_model.md`).

pub use holochain_zome_types::dht_v2::*;

use holo_hash::{
    hash_type, ActionHash, AnyLinkableHash, DhtOpHash, EntryHash, HasHash, HashableContent,
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

impl ChainOpUniqueForm<'_> {
    /// The content-derived [`DhtOpHash`] for an op of `op_type` carrying
    /// `action`, without needing a full [`ChainOp`] in hand.
    ///
    /// Agrees with [`ChainOp::to_hash`] for the corresponding op; used when
    /// producing the several ops a single record generates.
    pub fn op_hash(op_type: ChainOpType, action: &Action) -> DhtOpHash {
        let form = match op_type {
            ChainOpType::StoreRecord => ChainOpUniqueForm::CreateRecord(action),
            ChainOpType::StoreEntry => ChainOpUniqueForm::CreateEntry(action),
            ChainOpType::RegisterAgentActivity => ChainOpUniqueForm::AgentActivity(action),
            ChainOpType::RegisterUpdatedContent => ChainOpUniqueForm::UpdateEntry(action),
            ChainOpType::RegisterUpdatedRecord => ChainOpUniqueForm::UpdateRecord(action),
            ChainOpType::RegisterDeletedBy => ChainOpUniqueForm::DeleteRecord(action),
            ChainOpType::RegisterDeletedEntryAction => ChainOpUniqueForm::DeleteEntry(action),
            ChainOpType::RegisterAddLink => ChainOpUniqueForm::CreateLink(action),
            ChainOpType::RegisterRemoveLink => ChainOpUniqueForm::DeleteLink(action),
        };
        DhtOpHash::with_data_sync(&form)
    }
}

/// The set of op types a record with this action produces.
///
/// Mirrors the legacy `action_to_op_types`, matched over the v2 [`ActionData`].
pub fn action_to_op_types(action: &Action) -> Vec<ChainOpType> {
    use ChainOpType::*;
    match &action.data {
        ActionData::Dna(_)
        | ActionData::OpenChain(_)
        | ActionData::CloseChain(_)
        | ActionData::AgentValidationPkg(_)
        | ActionData::InitZomesComplete(_) => vec![StoreRecord, RegisterAgentActivity],
        ActionData::CreateLink(_) => vec![StoreRecord, RegisterAgentActivity, RegisterAddLink],
        ActionData::DeleteLink(_) => vec![StoreRecord, RegisterAgentActivity, RegisterRemoveLink],
        ActionData::Create(_) => vec![StoreRecord, RegisterAgentActivity, StoreEntry],
        ActionData::Update(_) => vec![
            StoreRecord,
            RegisterAgentActivity,
            StoreEntry,
            RegisterUpdatedContent,
            RegisterUpdatedRecord,
        ],
        ActionData::Delete(_) => vec![
            StoreRecord,
            RegisterAgentActivity,
            RegisterDeletedBy,
            RegisterDeletedEntryAction,
        ],
    }
}

/// The DHT basis where an op of `op_type` for `action` (hashed as
/// `action_hash`) is stored, mirroring the legacy `ChainOpUniqueForm::basis`.
///
/// Returns `None` only for an `op_type` that does not match the action's data,
/// which [`action_to_op_types`] never emits.
#[allow(dead_code)] // will be wired in the next task (produce_ops_from_record)
fn op_basis(
    op_type: ChainOpType,
    action_hash: &ActionHash,
    action: &Action,
) -> Option<AnyLinkableHash> {
    use ChainOpType::*;
    Some(match (op_type, &action.data) {
        (StoreRecord, _) => action_hash.clone().into(),
        (RegisterAgentActivity, _) => action.header.author.clone().into(),
        (StoreEntry, ActionData::Create(d)) => d.entry_hash.clone().into(),
        (StoreEntry, ActionData::Update(d)) => d.entry_hash.clone().into(),
        (RegisterUpdatedContent, ActionData::Update(d)) => d.original_entry_address.clone().into(),
        (RegisterUpdatedRecord, ActionData::Update(d)) => d.original_action_address.clone().into(),
        (RegisterDeletedBy, ActionData::Delete(d)) => d.deletes_address.clone().into(),
        (RegisterDeletedEntryAction, ActionData::Delete(d)) => {
            d.deletes_entry_address.clone().into()
        }
        (RegisterAddLink, ActionData::CreateLink(d)) => d.base_address.clone(),
        (RegisterRemoveLink, ActionData::DeleteLink(d)) => d.base_address.clone(),
        _ => return None,
    })
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
    ///
    /// `AnyLinkableHash`, not `AnyDhtHash`: link-op bases may be `External`
    /// hashes, which `AnyDhtHash` cannot hold (matches `InsertChainOp`).
    pub basis_hash: AnyLinkableHash,
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

    #[test]
    fn op_hash_entry_point_matches_chain_op_to_hash() {
        let sa = signed(create_action(), 7);
        // Cover every variant: a v2 `ChainOp` accepts any `SignedAction`
        // regardless of the action's content, so the same `sa` exercises the
        // full `to_unique_form` / `op_hash` mapping.
        let cases = [
            (
                ChainOp::CreateRecord(sa.clone(), OpEntry::ActionOnly),
                ChainOpType::StoreRecord,
            ),
            (
                ChainOp::CreateEntry(sa.clone(), OpEntry::ActionOnly),
                ChainOpType::StoreEntry,
            ),
            (
                ChainOp::AgentActivity(sa.clone()),
                ChainOpType::RegisterAgentActivity,
            ),
            (
                ChainOp::UpdateEntry(sa.clone(), OpEntry::ActionOnly),
                ChainOpType::RegisterUpdatedContent,
            ),
            (
                ChainOp::UpdateRecord(sa.clone(), OpEntry::ActionOnly),
                ChainOpType::RegisterUpdatedRecord,
            ),
            (
                ChainOp::DeleteRecord(sa.clone()),
                ChainOpType::RegisterDeletedBy,
            ),
            (
                ChainOp::DeleteEntry(sa.clone()),
                ChainOpType::RegisterDeletedEntryAction,
            ),
            (
                ChainOp::CreateLink(sa.clone()),
                ChainOpType::RegisterAddLink,
            ),
            (
                ChainOp::DeleteLink(sa.clone()),
                ChainOpType::RegisterRemoveLink,
            ),
        ];
        for (op, op_type) in cases {
            assert_eq!(op.to_hash(), ChainOpUniqueForm::op_hash(op_type, sa.data()));
        }
    }

    #[test]
    fn action_to_op_types_create_produces_record_activity_entry() {
        let action = create_action();
        assert_eq!(
            action_to_op_types(&action),
            vec![
                ChainOpType::StoreRecord,
                ChainOpType::RegisterAgentActivity,
                ChainOpType::StoreEntry,
            ]
        );
    }

    #[test]
    fn op_basis_uses_action_hash_for_store_record_and_entry_hash_for_store_entry() {
        use holo_hash::AnyLinkableHash;
        let action = create_action();
        let action_hash = ActionHash::from_raw_36(vec![8u8; 36]);

        let record_basis = op_basis(ChainOpType::StoreRecord, &action_hash, &action).unwrap();
        assert_eq!(record_basis, AnyLinkableHash::from(action_hash.clone()));

        let entry_basis = op_basis(ChainOpType::StoreEntry, &action_hash, &action).unwrap();
        assert_eq!(
            entry_basis,
            AnyLinkableHash::from(EntryHash::from_raw_36(vec![3u8; 36]))
        );
    }
}

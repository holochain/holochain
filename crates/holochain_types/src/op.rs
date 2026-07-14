//! DHT state-model op types (see `docs/design/state_model.md`).

use holo_hash::{
    hash_type, ActionHash, AgentPubKey, AnyLinkableHash, DhtOpHash, EntryHash, HasHash,
    HashableContent, HashableContentBytes, HoloHashed,
};
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::*;
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

/// Top-level DHT op.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub enum DhtOp {
    /// A chain-level op (record/entry/activity/link).
    ChainOp(Box<ChainOp>),
    /// A warrant op.
    WarrantOp(Box<crate::warrant::WarrantOp>),
}

impl From<ChainOp> for DhtOp {
    fn from(op: ChainOp) -> Self {
        DhtOp::ChainOp(Box::new(op))
    }
}

impl From<crate::warrant::WarrantOp> for DhtOp {
    fn from(op: crate::warrant::WarrantOp) -> Self {
        DhtOp::WarrantOp(Box::new(op))
    }
}

impl From<SignedWarrant> for DhtOp {
    fn from(op: SignedWarrant) -> Self {
        DhtOp::WarrantOp(Box::new(crate::warrant::WarrantOp::from(op)))
    }
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
/// Every variant borrows `&Action` directly, because `Action` is a single flat
/// struct.
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
    /// Build a [`ChainOp`] from its op type, signed action, and the [`OpEntry`]
    /// to carry for the entry-bearing variants (the link, delete, and
    /// agent-activity variants ignore `op_entry`).
    pub fn from_type(
        op_type: holochain_zome_types::op::ChainOpType,
        signed_action: SignedAction,
        op_entry: OpEntry,
    ) -> Self {
        use holochain_zome_types::op::ChainOpType;
        match op_type {
            ChainOpType::StoreRecord => ChainOp::CreateRecord(signed_action, op_entry),
            ChainOpType::StoreEntry => ChainOp::CreateEntry(signed_action, op_entry),
            ChainOpType::RegisterAgentActivity => ChainOp::AgentActivity(signed_action),
            ChainOpType::RegisterUpdatedContent => ChainOp::UpdateEntry(signed_action, op_entry),
            ChainOpType::RegisterUpdatedRecord => ChainOp::UpdateRecord(signed_action, op_entry),
            ChainOpType::RegisterDeletedEntryAction => ChainOp::DeleteEntry(signed_action),
            ChainOpType::RegisterDeletedBy => ChainOp::DeleteRecord(signed_action),
            ChainOpType::RegisterAddLink => ChainOp::CreateLink(signed_action),
            ChainOpType::RegisterRemoveLink => ChainOp::DeleteLink(signed_action),
        }
    }

    /// The content-derived [`DhtOpHash`] for this op.
    ///
    /// Excludes the signature and entry (see [`ChainOpUniqueForm`]), so it is
    /// reproducible from the op alone.
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

    /// The [`ChainOpType`] discriminant of this op.
    pub fn op_type(&self) -> ChainOpType {
        match self {
            ChainOp::CreateRecord(..) => ChainOpType::StoreRecord,
            ChainOp::CreateEntry(..) => ChainOpType::StoreEntry,
            ChainOp::AgentActivity(..) => ChainOpType::RegisterAgentActivity,
            ChainOp::UpdateEntry(..) => ChainOpType::RegisterUpdatedContent,
            ChainOp::UpdateRecord(..) => ChainOpType::RegisterUpdatedRecord,
            ChainOp::DeleteEntry(..) => ChainOpType::RegisterDeletedEntryAction,
            ChainOp::DeleteRecord(..) => ChainOpType::RegisterDeletedBy,
            ChainOp::CreateLink(..) => ChainOpType::RegisterAddLink,
            ChainOp::DeleteLink(..) => ChainOpType::RegisterRemoveLink,
        }
    }

    /// The signed action carried by this op.
    pub fn signed_action(&self) -> &SignedAction {
        match self {
            ChainOp::CreateRecord(sa, _)
            | ChainOp::CreateEntry(sa, _)
            | ChainOp::UpdateEntry(sa, _)
            | ChainOp::UpdateRecord(sa, _) => sa,
            ChainOp::AgentActivity(sa)
            | ChainOp::DeleteEntry(sa)
            | ChainOp::DeleteRecord(sa)
            | ChainOp::CreateLink(sa)
            | ChainOp::DeleteLink(sa) => sa,
        }
    }

    /// The op's entry payload, for the variants that carry one.
    pub fn op_entry(&self) -> Option<&OpEntry> {
        match self {
            ChainOp::CreateRecord(_, e)
            | ChainOp::CreateEntry(_, e)
            | ChainOp::UpdateEntry(_, e)
            | ChainOp::UpdateRecord(_, e) => Some(e),
            ChainOp::AgentActivity(_)
            | ChainOp::DeleteEntry(_)
            | ChainOp::DeleteRecord(_)
            | ChainOp::CreateLink(_)
            | ChainOp::DeleteLink(_) => None,
        }
    }

    /// The DHT basis where this op is stored.
    pub fn dht_basis(&self) -> AnyLinkableHash {
        let action = self.signed_action().data();
        let action_hash = ActionHash::with_data_sync(action);
        op_basis(self.op_type(), &action_hash, action)
            .expect("op_basis is total over an op paired with its own action")
    }

    /// Enzymatic countersigning session ops need special handling so that
    /// they arrive at the enzyme and not elsewhere. `None` when this isn't
    /// an enzymatic countersigning session, so it doubles as a boolean via
    /// `is_some()`.
    pub fn enzymatic_countersigning_enzyme(&self) -> Option<&AgentPubKey> {
        let OpEntry::Present(entry) = self.op_entry()? else {
            return None;
        };
        let Entry::CounterSign(session_data, _) = entry else {
            return None;
        };
        if session_data.preflight_request().enzymatic {
            session_data
                .preflight_request()
                .signing_agents
                .first()
                .map(|(pubkey, _)| pubkey)
        } else {
            None
        }
    }
}

impl DhtOp {
    /// The content-derived [`DhtOpHash`] for this op.
    pub fn to_hash(&self) -> DhtOpHash {
        match self {
            DhtOp::ChainOp(op) => op.to_hash(),
            // `WarrantOp` implements `HashableContent` directly.
            DhtOp::WarrantOp(op) => DhtOpHash::with_data_sync(op.as_ref()),
        }
    }

    /// The DHT basis where this op is stored.
    pub fn dht_basis(&self) -> AnyLinkableHash {
        match self {
            DhtOp::ChainOp(op) => op.dht_basis(),
            DhtOp::WarrantOp(op) => op.data().warrantee.clone().into(),
        }
    }
}

/// A [`ChainOp`] paired with its [`DhtOpHash`].
pub type ChainOpHashed = HoloHashed<ChainOp>;

/// A [`DhtOp`] paired with its [`DhtOpHash`].
pub type DhtOpHashed = HoloHashed<DhtOp>;

impl HashableContent for ChainOp {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        // `to_hash` already routes through `ChainOpUniqueForm`; reuse it
        // rather than re-serializing, mirroring `HoloHashed`'s own
        // prehashed-content pattern.
        HashableContentBytes::Prehashed39(self.to_hash().get_raw_39().to_vec())
    }
}

impl HashableContent for DhtOp {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Prehashed39(self.to_hash().get_raw_39().to_vec())
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
/// Matched over the action's [`ActionData`].
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
/// `action_hash`) is stored.
///
/// Returns `None` only for an `op_type` that does not match the action's data,
/// which [`action_to_op_types`] never emits.
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

/// Produce the [`HashedChainOp`]s a [`Record`] generates, with all hashes,
/// bases, and storage locations pre-computed.
///
/// The `StoreEntry` op is omitted whenever the entry is not present — whether
/// because it is private (`Hidden`), inapplicable (`NA`), or held without its
/// body (`NotStored`). Separately, the entry-bearing ops (`op_carries_entry`)
/// carry the entry payload only when it is present.
pub fn produce_ops_from_record(record: &Record) -> Vec<HashedChainOp> {
    let action = record.action();
    let action_hash = record.action_address();
    let entry = record.entry.as_option();

    let mut ops = Vec::new();
    for op_type in action_to_op_types(action) {
        // The store-entry op carries nothing without the entry.
        if op_type == ChainOpType::StoreEntry && entry.is_none() {
            continue;
        }
        let Some(basis_hash) = op_basis(op_type, action_hash, action) else {
            continue;
        };
        let op_entry = if op_carries_entry(op_type) {
            // Re-hashed from content; for a valid record this equals the
            // action's `entry_hash`.
            entry.map(|e| HoloHashed::from_content_sync(e.clone()))
        } else {
            None
        };
        ops.push(HashedChainOp {
            op_hash: ChainOpUniqueForm::op_hash(op_type, action),
            action: record.signed_action.clone(),
            entry: op_entry,
            op_type,
            storage_center_loc: basis_hash.get_loc(),
            basis_hash,
        });
    }
    ops
}

/// Whether an op of this type carries the record's entry payload.
fn op_carries_entry(op_type: ChainOpType) -> bool {
    matches!(
        op_type,
        ChainOpType::StoreRecord
            | ChainOpType::StoreEntry
            | ChainOpType::RegisterUpdatedContent
            | ChainOpType::RegisterUpdatedRecord
    )
}

/// Internal representation of a `ChainOp` with all hashes pre-computed.
/// Used during the incoming-ops workflow so hashes aren't recomputed for
/// each database write.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Build a [`HashedChainOp`] for a single op type from a signed action and
    /// optional entry, computing the op hash and DHT basis.
    ///
    /// Returns `None` when `op_type` does not correspond to the action's data
    /// (a mismatched op that [`action_to_op_types`] would never emit).
    pub fn from_signed_action(
        action: SignedActionHashed,
        op_type: ChainOpType,
        entry: Option<HoloHashed<Entry>>,
    ) -> Option<Self> {
        let op_hash = ChainOpUniqueForm::op_hash(op_type, action.action());
        let basis_hash = op_basis(op_type, action.as_hash(), action.action())?;
        Some(Self {
            op_hash,
            action,
            entry,
            op_type,
            storage_center_loc: basis_hash.get_loc(),
            basis_hash,
        })
    }

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
        // Cover every variant: a `ChainOp` accepts any `SignedAction`
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
    fn dht_op_hash_and_basis_delegate_to_chain_op() {
        let sa = signed(create_action(), 7);
        let chain_op = ChainOp::CreateRecord(sa.clone(), OpEntry::ActionOnly);
        let dht_op = DhtOp::ChainOp(Box::new(chain_op.clone()));

        // DhtOp::to_hash agrees with ChainOp::to_hash.
        assert_eq!(dht_op.to_hash(), chain_op.to_hash());

        // StoreRecord basis is the action hash.
        let action_hash = ActionHash::with_data_sync(sa.data());
        assert_eq!(dht_op.dht_basis(), AnyLinkableHash::from(action_hash));
        assert_eq!(dht_op.dht_basis(), chain_op.dht_basis());
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

#[cfg(test)]
mod produce_ops_tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, EntryHash, HoloHashed};
    use holochain_timestamp::Timestamp;
    use holochain_zome_types::prelude::{AppEntryDef, EntryType, EntryVisibility};
    use holochain_zome_types::record::{Record, RecordEntry, SignedHashed};
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

    fn update_action() -> Action {
        Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: Timestamp::from_micros(2_000),
                action_seq: 5,
                prev_action: Some(ActionHash::from_raw_36(vec![2u8; 36])),
            },
            data: ActionData::Update(UpdateData {
                original_action_address: ActionHash::from_raw_36(vec![6u8; 36]),
                original_entry_address: EntryHash::from_raw_36(vec![7u8; 36]),
                entry_type: EntryType::App(AppEntryDef::new(
                    0.into(),
                    0.into(),
                    EntryVisibility::Public,
                )),
                entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
            }),
        }
    }

    fn record(action: Action, entry: RecordEntry<Entry>) -> Record {
        let hashed = HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![9u8; 36]));
        Record::new(
            SignedHashed::with_presigned(hashed, Signature([7u8; 64])),
            entry,
        )
    }

    fn op_types(ops: &[HashedChainOp]) -> Vec<ChainOpType> {
        ops.iter().map(|o| o.op_type).collect()
    }

    #[test]
    fn create_with_public_entry_produces_record_activity_entry() {
        let entry = Entry::Agent(AgentPubKey::from_raw_36(vec![5u8; 36]));
        let ops = produce_ops_from_record(&record(create_action(), RecordEntry::Present(entry)));
        assert_eq!(
            op_types(&ops),
            vec![
                ChainOpType::StoreRecord,
                ChainOpType::RegisterAgentActivity,
                ChainOpType::StoreEntry,
            ]
        );
        let by_type = |t| ops.iter().find(|o| o.op_type == t).unwrap();
        assert!(by_type(ChainOpType::StoreRecord).entry.is_some());
        assert!(by_type(ChainOpType::StoreEntry).entry.is_some());
        assert!(by_type(ChainOpType::RegisterAgentActivity).entry.is_none());
    }

    #[test]
    fn create_with_hidden_entry_skips_store_entry_and_omits_payload() {
        let ops = produce_ops_from_record(&record(create_action(), RecordEntry::Hidden));
        assert_eq!(
            op_types(&ops),
            vec![ChainOpType::StoreRecord, ChainOpType::RegisterAgentActivity]
        );
        let store_record = ops
            .iter()
            .find(|o| o.op_type == ChainOpType::StoreRecord)
            .unwrap();
        assert!(store_record.entry.is_none());
    }

    #[test]
    fn store_record_basis_is_the_action_hash() {
        use holo_hash::AnyLinkableHash;
        let r = record(create_action(), RecordEntry::Hidden);
        let action_hash = r.action_address().clone();
        let ops = produce_ops_from_record(&r);
        let store_record = ops
            .iter()
            .find(|o| o.op_type == ChainOpType::StoreRecord)
            .unwrap();
        assert_eq!(store_record.basis_hash, AnyLinkableHash::from(action_hash));
    }

    #[test]
    fn update_with_public_entry_produces_full_op_set_with_update_payloads() {
        use holo_hash::AnyLinkableHash;
        let entry = Entry::Agent(AgentPubKey::from_raw_36(vec![5u8; 36]));
        let ops = produce_ops_from_record(&record(update_action(), RecordEntry::Present(entry)));
        assert_eq!(
            op_types(&ops),
            vec![
                ChainOpType::StoreRecord,
                ChainOpType::RegisterAgentActivity,
                ChainOpType::StoreEntry,
                ChainOpType::RegisterUpdatedContent,
                ChainOpType::RegisterUpdatedRecord,
            ]
        );
        let by_type = |t| ops.iter().find(|o| o.op_type == t).unwrap();
        // The update ops are entry-bearing and carry the new entry.
        assert!(by_type(ChainOpType::RegisterUpdatedContent).entry.is_some());
        assert!(by_type(ChainOpType::RegisterUpdatedRecord).entry.is_some());
        assert!(by_type(ChainOpType::RegisterAgentActivity).entry.is_none());
        // Update bases: content → original entry, record → original action.
        assert_eq!(
            by_type(ChainOpType::RegisterUpdatedContent).basis_hash,
            AnyLinkableHash::from(EntryHash::from_raw_36(vec![7u8; 36]))
        );
        assert_eq!(
            by_type(ChainOpType::RegisterUpdatedRecord).basis_hash,
            AnyLinkableHash::from(ActionHash::from_raw_36(vec![6u8; 36]))
        );
    }
}

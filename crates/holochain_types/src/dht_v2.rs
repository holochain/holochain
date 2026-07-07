//! Redesigned DHT state-model op types (transitional — see `docs/design/state_model.md`).

pub use holochain_zome_types::dht_v2::*;

use holo_hash::{
    hash_type, ActionHash, AgentPubKey, AnyLinkableHash, DhtOpHash, EntryHash, HasHash,
    HashableContent, HashableContentBytes, HoloHashed,
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
/// as a single sum type. Derefs to the wrapped [`SignedWarrant`] (which
/// itself derefs to `Warrant`/`WarrantProof`), matching the legacy
/// `holochain_types::warrant::WarrantOp`'s field-access ergonomics.
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, derive_more::Deref,
)]
pub struct WarrantOp(pub SignedWarrant);

impl WarrantOp {
    /// The wrapped warrant, matching the legacy
    /// `holochain_types::warrant::WarrantOp::warrant` accessor.
    pub fn warrant(&self) -> &holochain_zome_types::warrant::Warrant {
        self.0.data()
    }
}

/// Top-level DHT op.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub enum DhtOp {
    /// A chain-level op (record/entry/activity/link).
    ChainOp(Box<ChainOp>),
    /// A warrant op.
    WarrantOp(Box<WarrantOp>),
}

impl From<ChainOp> for DhtOp {
    fn from(op: ChainOp) -> Self {
        DhtOp::ChainOp(Box::new(op))
    }
}

impl From<WarrantOp> for DhtOp {
    fn from(op: WarrantOp) -> Self {
        DhtOp::WarrantOp(Box::new(op))
    }
}

impl From<SignedWarrant> for DhtOp {
    fn from(op: SignedWarrant) -> Self {
        DhtOp::WarrantOp(Box::new(WarrantOp(op)))
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
            // Warrant hashing is unchanged by the v2 flip; reuse the legacy
            // `WarrantOp` wrapper, which carries the `DhtOp` hash type over the
            // warrant content. The wrapped `SignedWarrant` is identical, so the
            // hash matches the legacy form.
            DhtOp::WarrantOp(op) => {
                DhtOpHash::with_data_sync(&crate::warrant::WarrantOp::from(op.0.clone()))
            }
        }
    }

    /// The DHT basis where this op is stored.
    pub fn dht_basis(&self) -> AnyLinkableHash {
        match self {
            DhtOp::ChainOp(op) => op.dht_basis(),
            DhtOp::WarrantOp(op) => op.0.data().warrantee.clone().into(),
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

/// Reconstruct a legacy [`crate::dht_op::DhtOp`] from a v2 [`DhtOp`].
///
/// The op pipeline itself is v2-native; this remains for the still-legacy
/// `DbKindDht` sqlite mirror (e.g. `incoming_dht_ops_workflow`'s dead
/// `add_to_pending`) and the dump-consistency tooling that reads it. The v2
/// action hash is preserved through [`to_legacy_signed_action`], so the
/// reconstructed op re-hashes to the same content-derived v2 identity.
pub fn to_legacy_dht_op(op: &DhtOp) -> crate::dht_op::DhtOpResult<crate::dht_op::DhtOp> {
    use crate::dht_op::{ChainOp as LegacyChainOp, DhtOp as LegacyDhtOp};
    match op {
        DhtOp::ChainOp(chain_op) => {
            let signed = chain_op.signed_action();
            // The from_content_sync hash is the canonical v2 hash, which
            // to_legacy_signed_action then carries onto the legacy form.
            let hashed = HoloHashed::from_content_sync(signed.data().clone());
            let v2_sah = holochain_zome_types::record::SignedHashed::with_presigned(
                hashed,
                signed.signature().clone(),
            );
            let legacy_sah = to_legacy_signed_action(&v2_sah);
            let entry = chain_op.op_entry().and_then(|e| match e {
                OpEntry::Present(entry) => Some(entry.clone()),
                OpEntry::Hidden | OpEntry::ActionOnly => None,
            });
            // `LegacyChainOp::from_type` takes a `LegacySignedAction`
            // (`Signed<legacy Action>`, unhashed — it only stores the action
            // and signature, not the hash), so drop the hash carried on
            // `legacy_sah` here.
            let (legacy_hashed, legacy_signature) = legacy_sah.into_inner();
            let legacy_signed_action = crate::dht_op::LegacySignedAction::new(
                legacy_hashed.into_content(),
                legacy_signature,
            );
            let legacy = LegacyChainOp::from_type(chain_op.op_type(), legacy_signed_action, entry)?;
            Ok(LegacyDhtOp::ChainOp(Box::new(legacy)))
        }
        DhtOp::WarrantOp(w) => Ok(LegacyDhtOp::WarrantOp(Box::new(
            crate::warrant::WarrantOp::from(w.0.clone()),
        ))),
    }
}

/// Reconstruct a v2 [`DhtOpHashed`] from a legacy [`crate::dht_op::DhtOpHashed`].
///
/// The inverse of [`to_legacy_dht_op`]: used where a legacy-only data source
/// (e.g. the sqlite `DbKindCache` warrant-dependency mirror) still hands back
/// a legacy op that must be fed into the v2 op pipeline
/// (`DhtStore::record_incoming_ops`). The legacy op hash IS the v2
/// content-derived hash (see the module-level safety invariant), so this
/// preserves it via `with_pre_hashed` rather than re-hashing.
pub fn from_legacy_dht_op(op: &crate::dht_op::DhtOpHashed) -> DhtOpHashed {
    use crate::dht_op::DhtOp as LegacyDhtOp;
    use holochain_zome_types::prelude::RecordEntryRef;

    let hash = op.as_hash().clone();
    let v2_op = match op.as_content() {
        LegacyDhtOp::ChainOp(chain_op) => {
            let legacy_signed = chain_op.signed_action();
            let v2_action = from_legacy_action(legacy_signed.data());
            let signed_action = SignedAction::new(v2_action, legacy_signed.signature().clone());
            let op_entry = |e: RecordEntryRef<'_>| -> OpEntry {
                match e {
                    RecordEntryRef::Present(entry) => OpEntry::Present(entry.clone()),
                    RecordEntryRef::Hidden => OpEntry::Hidden,
                    RecordEntryRef::NA | RecordEntryRef::NotStored => OpEntry::ActionOnly,
                }
            };
            let v2_chain_op = match chain_op.get_type() {
                ChainOpType::StoreRecord => {
                    ChainOp::CreateRecord(signed_action, op_entry(chain_op.entry()))
                }
                ChainOpType::StoreEntry => {
                    ChainOp::CreateEntry(signed_action, op_entry(chain_op.entry()))
                }
                ChainOpType::RegisterAgentActivity => ChainOp::AgentActivity(signed_action),
                ChainOpType::RegisterUpdatedContent => {
                    ChainOp::UpdateEntry(signed_action, op_entry(chain_op.entry()))
                }
                ChainOpType::RegisterUpdatedRecord => {
                    ChainOp::UpdateRecord(signed_action, op_entry(chain_op.entry()))
                }
                ChainOpType::RegisterDeletedBy => ChainOp::DeleteRecord(signed_action),
                ChainOpType::RegisterDeletedEntryAction => ChainOp::DeleteEntry(signed_action),
                ChainOpType::RegisterAddLink => ChainOp::CreateLink(signed_action),
                ChainOpType::RegisterRemoveLink => ChainOp::DeleteLink(signed_action),
            };
            DhtOp::ChainOp(Box::new(v2_chain_op))
        }
        LegacyDhtOp::WarrantOp(w) => DhtOp::WarrantOp(Box::new(WarrantOp(SignedWarrant::new(
            w.warrant().clone(),
            w.signature().clone(),
        )))),
    };
    DhtOpHashed::with_pre_hashed(v2_op, hash)
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

/// Produce the [`HashedChainOp`]s a v2 [`Record`] generates, with all hashes,
/// bases, and storage locations pre-computed — the v2 analog of the legacy
/// `produce_ops_from_record`.
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
    fn v2_hashable_content_matches_legacy_chain_op_hash() {
        use crate::dht_op::ChainOp as LegacyChainOp;
        use holochain_zome_types::dependencies::holochain_integrity_types::action::{
            Action as LegacyAction, Create as LegacyCreate,
        };
        use holochain_zome_types::record::RecordEntry;

        let legacy_action = LegacyAction::Create(LegacyCreate {
            author: AgentPubKey::from_raw_36(vec![1u8; 36]),
            timestamp: Timestamp::from_micros(1_000),
            action_seq: 4,
            prev_action: ActionHash::from_raw_36(vec![2u8; 36]),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
            weight: Default::default(),
        });
        let legacy_op = LegacyChainOp::StoreRecord(
            Signature::from([7u8; 64]),
            legacy_action.clone(),
            RecordEntry::NA,
        );
        let legacy_hash = DhtOpHash::with_data_sync(&legacy_op);

        let v2_action = from_legacy_action(&legacy_action);
        let v2_op = ChainOp::CreateRecord(signed(v2_action, 7), OpEntry::ActionOnly);

        // `ChainOp::to_hash` (used by `dht_basis`/production) agrees with the
        // legacy op's content-derived hash for the same action.
        assert_eq!(v2_op.to_hash(), legacy_hash);

        // `HashableContent` (used by `HoloHashed::from_content_sync`, i.e.
        // `DhtOpHashed`/`ChainOpHashed`) agrees too.
        let dht_op = DhtOp::ChainOp(Box::new(v2_op.clone()));
        let hashed: DhtOpHashed = HoloHashed::from_content_sync(dht_op);
        assert_eq!(*hashed.as_hash(), legacy_hash);
        let chain_hashed: ChainOpHashed = HoloHashed::from_content_sync(v2_op);
        assert_eq!(*chain_hashed.as_hash(), legacy_hash);
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
    use holochain_zome_types::dht_v2::Record;
    use holochain_zome_types::prelude::{AppEntryDef, EntryType, EntryVisibility};
    use holochain_zome_types::record::{RecordEntry, SignedHashed};
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

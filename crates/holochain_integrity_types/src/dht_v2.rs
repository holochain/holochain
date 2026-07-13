//! DHT state-model types (see `docs/design/state_model.md`).
//!
//! The action model is a common [`ActionHeader`] + per-variant `*Data` struct,
//! pulled together by a tagged [`ActionData`] enum. The resulting [`Action`] is
//! content-only and always hashed via [`holo_hash::HoloHashed`] /
//! [`SignedHashed`] at call sites, so the stored hash is invariant with the
//! content.
//!
//! [`SignedHashed`]: crate::record::SignedHashed

pub mod op;
pub mod record;

pub use op::{
    Op, RegisterAgentActivity, RegisterCreateLink, RegisterDelete, RegisterDeleteLink,
    RegisterUpdate, StoreEntry, StoreRecord,
};
pub use record::Record;

use crate::action::ZomeIndex;
use crate::entry_def::EntryVisibility;
use crate::{
    link::{LinkTag, LinkType},
    AppEntryDef, EntryType, MembraneProof,
};
use holo_hash::{
    ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash, HashableContent,
    HashableContentBytes, HoloHashed,
};
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;

/// Record-level validation outcome.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum RecordValidity {
    /// The record was accepted.
    Accepted = 1,
    /// The record was rejected.
    Rejected = 2,
}

/// Alias for the validation status of a DHT op (chain op or warrant).
///
/// Semantically identical to [`RecordValidity`] — `Accepted` or `Rejected` —
/// but named for ops rather than records to make call-site intent clearer.
pub type OpValidity = RecordValidity;

/// Maps [`RecordValidity`] onto the `record_validity` /
/// `sys_validation_status` / `app_validation_status` INTEGER columns
/// (`1 = Accepted`, `2 = Rejected`). `0` is reserved and never written.
/// `NULL` represents pending and is decoded at the column boundary, not
/// via this impl.
impl From<RecordValidity> for i64 {
    fn from(v: RecordValidity) -> Self {
        v as i64
    }
}

/// Inverse of [`From<RecordValidity> for i64`]. Returns `Err(v)` for any
/// value outside `1..=2` (including `0`).
impl TryFrom<i64> for RecordValidity {
    type Error = i64;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(RecordValidity::Accepted),
            2 => Ok(RecordValidity::Rejected),
            other => Err(other),
        }
    }
}

/// Action-type discriminant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
#[repr(i64)]
pub enum ActionType {
    /// Genesis DNA action. Always `action_seq == 0` and `prev_action == None`.
    Dna = 1,
    /// Agent validation package (membership proof) action.
    AgentValidationPkg = 2,
    /// Marker action emitted once all init zomes have completed.
    InitZomesComplete = 3,
    /// Creates a new entry on the chain.
    Create = 4,
    /// Updates an existing entry with new content.
    Update = 5,
    /// Deletes an existing entry.
    Delete = 6,
    /// Creates a link between two linkable addresses.
    CreateLink = 7,
    /// Deletes an existing `CreateLink` action.
    DeleteLink = 8,
    /// Closes the source chain, optionally pointing at a migration target.
    CloseChain = 9,
    /// Opens a new source chain following a migration from a previous chain.
    OpenChain = 10,
}

/// Maps [`ActionType`] onto the schema `Action.action_type` INTEGER column
/// (`1..=10`). `0` is reserved and never written.
impl From<ActionType> for i64 {
    fn from(t: ActionType) -> Self {
        t as i64
    }
}

/// Inverse of [`From<ActionType> for i64`]. Returns `Err(v)` for any value
/// outside `1..=10` (including `0`).
impl TryFrom<i64> for ActionType {
    type Error = i64;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        use ActionType::*;
        match v {
            1 => Ok(Dna),
            2 => Ok(AgentValidationPkg),
            3 => Ok(InitZomesComplete),
            4 => Ok(Create),
            5 => Ok(Update),
            6 => Ok(Delete),
            7 => Ok(CreateLink),
            8 => Ok(DeleteLink),
            9 => Ok(CloseChain),
            10 => Ok(OpenChain),
            other => Err(other),
        }
    }
}

impl core::fmt::Display for ActionType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match self {
            ActionType::Dna => "Dna",
            ActionType::AgentValidationPkg => "AgentValidationPkg",
            ActionType::InitZomesComplete => "InitZomesComplete",
            ActionType::Create => "Create",
            ActionType::Update => "Update",
            ActionType::Delete => "Delete",
            ActionType::CreateLink => "CreateLink",
            ActionType::DeleteLink => "DeleteLink",
            ActionType::CloseChain => "CloseChain",
            ActionType::OpenChain => "OpenChain",
        };
        f.write_str(name)
    }
}

/// Capability-grant access mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum CapAccess {
    /// No restrictions; any caller may invoke the granted function.
    Unrestricted = 0,
    /// Caller must present a matching secret token.
    Transferable = 1,
    /// Caller must be on the agent allow-list and present the secret.
    Assigned = 2,
}

/// Maps [`CapAccess`] onto the `CapGrant.cap_access` INTEGER column
/// (`0..=2`). All three variants are valid, including `0`.
impl From<CapAccess> for i64 {
    fn from(a: CapAccess) -> Self {
        a as i64
    }
}

/// Inverse of [`From<CapAccess> for i64`]. Returns `Err(v)` for any value
/// outside `0..=2`.
impl TryFrom<i64> for CapAccess {
    type Error = i64;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(CapAccess::Unrestricted),
            1 => Ok(CapAccess::Transferable),
            2 => Ok(CapAccess::Assigned),
            other => Err(other),
        }
    }
}

/// Common header fields shared by every action variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct ActionHeader {
    /// The agent who authored this action.
    pub author: AgentPubKey,
    /// Microsecond timestamp at which the action was authored.
    pub timestamp: Timestamp,
    /// The action's position on the authoring agent's source chain.
    pub action_seq: u32,
    /// The preceding action's hash on the source chain.
    ///
    /// `None` only for the genesis [`ActionData::Dna`] action.
    pub prev_action: Option<ActionHash>,
}

/// Per-variant data for [`ActionType::Dna`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct DnaData {
    /// Hash of the DNA that this chain is an instance of.
    pub dna_hash: DnaHash,
}

/// Per-variant data for [`ActionType::AgentValidationPkg`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct AgentValidationPkgData {
    /// Optional membrane proof provided when joining the network.
    pub membrane_proof: Option<MembraneProof>,
}

/// Per-variant data for [`ActionType::InitZomesComplete`].
///
/// Carries no payload — the variant alone signals that all init zomes ran.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct InitZomesCompleteData {}

/// Per-variant data for [`ActionType::Create`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct CreateData {
    /// Application-defined entry type (including visibility).
    pub entry_type: EntryType,
    /// Hash of the entry content being created.
    pub entry_hash: EntryHash,
}

/// Per-variant data for [`ActionType::Update`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct UpdateData {
    /// Hash of the action being updated.
    pub original_action_address: ActionHash,
    /// Hash of the original entry being updated.
    pub original_entry_address: EntryHash,
    /// Application-defined entry type of the new entry.
    pub entry_type: EntryType,
    /// Hash of the new entry content.
    pub entry_hash: EntryHash,
}

/// Per-variant data for [`ActionType::Delete`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct DeleteData {
    /// Hash of the action being deleted.
    pub deletes_address: ActionHash,
    /// Hash of the entry referenced by the deleted action.
    pub deletes_entry_address: EntryHash,
}

/// Per-variant data for [`ActionType::CreateLink`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct CreateLinkData {
    /// The hash the link points from.
    pub base_address: AnyLinkableHash,
    /// The hash the link points to.
    pub target_address: AnyLinkableHash,
    /// Index of the zome that defined this link type.
    pub zome_index: ZomeIndex,
    /// Link type identifier within the zome.
    pub link_type: LinkType,
    /// Opaque tag attached to the link.
    pub tag: LinkTag,
}

/// Per-variant data for [`ActionType::DeleteLink`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct DeleteLinkData {
    /// The base address of the link being removed.
    pub base_address: AnyLinkableHash,
    /// Hash of the `CreateLink` action being deleted.
    pub link_add_address: ActionHash,
}

/// Per-variant data for [`ActionType::CloseChain`].
///
/// The `author`, `timestamp`, `action_seq`, and `prev_action` fields live on
/// [`ActionHeader`]; only the chain-close-specific fields go here.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct CloseChainData {
    /// Optional migration target the chain closes towards.
    pub new_target: Option<crate::action::MigrationTarget>,
}

/// Per-variant data for [`ActionType::OpenChain`].
///
/// The `author`, `timestamp`, `action_seq`, and `prev_action` fields live on
/// [`ActionHeader`]; only the chain-open-specific fields go here.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct OpenChainData {
    /// The previous DNA hash or agent key this chain migrated from.
    pub prev_target: crate::action::MigrationTarget,
    /// Hash of the matching `CloseChain` action on the old chain.
    pub close_hash: ActionHash,
}

/// Per-variant action data, stored serialized in the `Action.action_data`
/// BLOB column.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
#[serde(tag = "type")]
pub enum ActionData {
    /// Genesis DNA action.
    Dna(DnaData),
    /// Agent validation package.
    AgentValidationPkg(AgentValidationPkgData),
    /// Signals that init zomes have completed.
    InitZomesComplete(InitZomesCompleteData),
    /// Creates a new entry.
    Create(CreateData),
    /// Updates an existing entry.
    Update(UpdateData),
    /// Deletes an existing entry.
    Delete(DeleteData),
    /// Creates a link between two addresses.
    CreateLink(CreateLinkData),
    /// Deletes a previously created link.
    DeleteLink(DeleteLinkData),
    /// Closes the source chain.
    CloseChain(CloseChainData),
    /// Opens a new chain following a migration.
    OpenChain(OpenChainData),
}

impl ActionData {
    /// The [`ActionType`] discriminant of this variant.
    pub fn action_type(&self) -> ActionType {
        match self {
            ActionData::Dna(_) => ActionType::Dna,
            ActionData::AgentValidationPkg(_) => ActionType::AgentValidationPkg,
            ActionData::InitZomesComplete(_) => ActionType::InitZomesComplete,
            ActionData::Create(_) => ActionType::Create,
            ActionData::Update(_) => ActionType::Update,
            ActionData::Delete(_) => ActionType::Delete,
            ActionData::CreateLink(_) => ActionType::CreateLink,
            ActionData::DeleteLink(_) => ActionType::DeleteLink,
            ActionData::CloseChain(_) => ActionType::CloseChain,
            ActionData::OpenChain(_) => ActionType::OpenChain,
        }
    }

    /// The action's referenced entry hash, if any.
    pub fn entry_hash(&self) -> Option<&EntryHash> {
        match self {
            ActionData::Create(d) => Some(&d.entry_hash),
            ActionData::Update(d) => Some(&d.entry_hash),
            _ => None,
        }
    }
}

/// Full action content: header + per-variant data.
///
/// The hash is not stored on the struct — use [`holo_hash::HoloHashed<Action>`]
/// (or [`crate::record::SignedHashed<Action>`]) at call sites so the hash is
/// always derived from the content.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct Action {
    /// Header fields common to every action variant.
    pub header: ActionHeader,
    /// The per-variant payload.
    pub data: ActionData,
}

impl Action {
    /// The public key of the agent who "authored" this action.
    ///
    /// This is not necessarily the agent who signed the action; see
    /// [`Action::signer`].
    pub fn author(&self) -> &AgentPubKey {
        &self.header.author
    }

    /// The public key of the agent who signed this action.
    ///
    /// This is not necessarily the agent who "authored" the action: a
    /// `CloseChain` action during an agent-key migration is signed with the
    /// new key rather than the author key, because the new key must be
    /// known in order for the migration to be effective.
    pub fn signer(&self) -> &AgentPubKey {
        match &self.data {
            ActionData::CloseChain(CloseChainData {
                new_target: Some(crate::action::MigrationTarget::Agent(agent)),
            }) => agent,
            _ => self.author(),
        }
    }

    /// The microsecond timestamp at which this action was authored.
    pub fn timestamp(&self) -> Timestamp {
        self.header.timestamp
    }

    /// This action's position on the authoring agent's source chain.
    pub fn action_seq(&self) -> u32 {
        self.header.action_seq
    }

    /// The hash of the preceding action on the source chain.
    ///
    /// `None` only for the genesis `Dna` action.
    pub fn prev_action(&self) -> Option<&ActionHash> {
        self.header.prev_action.as_ref()
    }

    /// A mutable reference to the preceding action hash.
    ///
    /// `None` only for the genesis `Dna` action.
    pub fn prev_action_mut(&mut self) -> Option<&mut ActionHash> {
        self.header.prev_action.as_mut()
    }

    /// `true` if this action's sequence number falls within the genesis
    /// portion of the chain.
    pub fn is_genesis(&self) -> bool {
        self.action_seq() < crate::action::POST_GENESIS_SEQ_THRESHOLD
    }

    /// The [`ActionType`] discriminant of this action.
    pub fn action_type(&self) -> ActionType {
        self.data.action_type()
    }

    /// The hash of the entry this action references, if any.
    pub fn entry_hash(&self) -> Option<&EntryHash> {
        self.data.entry_hash()
    }

    /// The type of the entry this action references, if any.
    pub fn entry_type(&self) -> Option<&EntryType> {
        match &self.data {
            ActionData::Create(d) => Some(&d.entry_type),
            ActionData::Update(d) => Some(&d.entry_type),
            _ => None,
        }
    }

    /// A mutable reference to the type of the entry this action references,
    /// for the `Create` and `Update` variants.
    pub fn entry_type_mut(&mut self) -> Option<&mut EntryType> {
        match &mut self.data {
            ActionData::Create(d) => Some(&mut d.entry_type),
            ActionData::Update(d) => Some(&mut d.entry_type),
            _ => None,
        }
    }

    /// A mutable reference to the hash of the entry this action references,
    /// for the `Create` and `Update` variants.
    pub fn entry_hash_mut(&mut self) -> Option<&mut EntryHash> {
        match &mut self.data {
            ActionData::Create(d) => Some(&mut d.entry_hash),
            ActionData::Update(d) => Some(&mut d.entry_hash),
            _ => None,
        }
    }

    /// The [`AppEntryDef`] of the entry this action references, if it is an
    /// application-defined entry.
    pub fn app_entry_def(&self) -> Option<&AppEntryDef> {
        match self.entry_type()? {
            EntryType::App(app_entry_def) => Some(app_entry_def),
            _ => None,
        }
    }

    /// The hash and type of the entry this action references, if any.
    pub fn entry_data(&self) -> Option<(&EntryHash, &EntryType)> {
        match &self.data {
            ActionData::Create(d) => Some((&d.entry_hash, &d.entry_type)),
            ActionData::Update(d) => Some((&d.entry_hash, &d.entry_type)),
            _ => None,
        }
    }

    /// Pulls the entry hash and type out of this action by value, if any.
    pub fn into_entry_data(self) -> Option<(EntryHash, EntryType)> {
        match self.data {
            ActionData::Create(d) => Some((d.entry_hash, d.entry_type)),
            ActionData::Update(d) => Some((d.entry_hash, d.entry_type)),
            _ => None,
        }
    }

    /// The visibility of the entry this action references, if any.
    pub fn entry_visibility(&self) -> Option<&EntryVisibility> {
        self.entry_type().map(|entry_type| entry_type.visibility())
    }
}

impl HashableContent for Action {
    type HashType = holo_hash::hash_type::Action;

    fn hash_type(&self) -> Self::HashType {
        use holo_hash::PrimitiveHashType;
        Self::HashType::new()
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            SerializedBytes::try_from(self).expect("Could not serialize v2 Action"),
        )
    }
}

/// An [`Action`] paired with its [`ActionHash`].
///
/// The agent-activity traits
/// [`ActionSequenceAndHash`](crate::action::ActionSequenceAndHash) and
/// [`ActionHashedContainer`](crate::action::ActionHashedContainer) are
/// implemented for it.
pub type ActionHashed = HoloHashed<Action>;

/// An [`Action`] that is both hashed and signed.
pub type SignedActionHashed = crate::record::SignedHashed<Action>;

impl SignedActionHashed {
    /// The action content.
    pub fn action(&self) -> &Action {
        &self.hashed.content
    }

    /// The action hash.
    pub fn action_address(&self) -> &ActionHash {
        &self.hashed.hash
    }
}

impl crate::action::ActionSequenceAndHash for ActionHashed {
    fn action_seq(&self) -> u32 {
        self.content.action_seq()
    }

    fn address(&self) -> &ActionHash {
        &self.hash
    }
}

impl crate::action::ActionHashedContainer for ActionHashed {
    fn action(&self) -> &Action {
        &self.content
    }

    fn action_hash(&self) -> &ActionHash {
        &self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_validity_i64_roundtrip() {
        for v in [RecordValidity::Accepted, RecordValidity::Rejected] {
            let n: i64 = v.into();
            assert_eq!(RecordValidity::try_from(n).unwrap(), v);
        }
        assert!(RecordValidity::try_from(0).is_err());
        assert!(RecordValidity::try_from(3).is_err());
    }

    #[test]
    fn action_type_i64_roundtrip() {
        use ActionType::*;
        for v in [
            Dna,
            AgentValidationPkg,
            InitZomesComplete,
            Create,
            Update,
            Delete,
            CreateLink,
            DeleteLink,
            CloseChain,
            OpenChain,
        ] {
            let n: i64 = v.into();
            assert_eq!(ActionType::try_from(n).unwrap(), v);
        }
        assert!(ActionType::try_from(0).is_err());
        assert!(ActionType::try_from(11).is_err());
    }

    #[test]
    fn cap_access_i64_roundtrip() {
        for v in [
            CapAccess::Unrestricted,
            CapAccess::Transferable,
            CapAccess::Assigned,
        ] {
            let n: i64 = v.into();
            assert_eq!(CapAccess::try_from(n).unwrap(), v);
        }
        assert!(CapAccess::try_from(-1).is_err());
        assert!(CapAccess::try_from(3).is_err());
    }

    #[test]
    fn data_structs_construct() {
        // Sanity check that each struct has the expected shape by constructing one.
        let _ = DnaData {
            dna_hash: DnaHash::from_raw_36(vec![0u8; 36]),
        };
        let _ = InitZomesCompleteData {};
    }

    #[test]
    fn action_data_serde_roundtrip() {
        let cases: Vec<ActionData> = vec![
            ActionData::Dna(DnaData {
                dna_hash: DnaHash::from_raw_36(vec![1u8; 36]),
            }),
            ActionData::InitZomesComplete(InitZomesCompleteData {}),
            ActionData::Create(CreateData {
                entry_type: EntryType::AgentPubKey,
                entry_hash: EntryHash::from_raw_36(vec![2u8; 36]),
            }),
        ];
        for data in cases {
            let bytes = holochain_serialized_bytes::encode(&data).unwrap();
            let decoded: ActionData = holochain_serialized_bytes::decode(&bytes).unwrap();
            assert_eq!(decoded.action_type(), data.action_type());
        }
    }

    fn sample_action(data: ActionData) -> Action {
        Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: Timestamp::from_micros(42),
                action_seq: 5,
                prev_action: Some(ActionHash::from_raw_36(vec![2u8; 36])),
            },
            data,
        }
    }

    fn sample_create_data() -> ActionData {
        ActionData::Create(CreateData {
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
        })
    }

    #[test]
    fn action_accessors_read_header_fields() {
        let a = sample_action(sample_create_data());
        assert_eq!(a.author(), &AgentPubKey::from_raw_36(vec![1u8; 36]));
        assert_eq!(a.timestamp(), Timestamp::from_micros(42));
        assert_eq!(a.action_seq(), 5);
        assert_eq!(
            a.prev_action(),
            Some(&ActionHash::from_raw_36(vec![2u8; 36]))
        );
        assert_eq!(a.action_type(), ActionType::Create);
    }

    #[test]
    fn action_prev_action_mut_writes_through_the_header() {
        let mut a = sample_action(sample_create_data());
        let new_prev = ActionHash::from_raw_36(vec![9u8; 36]);
        *a.prev_action_mut().expect("has a prev action") = new_prev.clone();
        assert_eq!(a.prev_action(), Some(&new_prev));
    }

    #[test]
    fn action_entry_type_and_data_some_for_create_and_update() {
        let create = sample_action(sample_create_data());
        assert_eq!(create.entry_type(), Some(&EntryType::AgentPubKey));
        assert_eq!(
            create.entry_data(),
            Some((
                &EntryHash::from_raw_36(vec![3u8; 36]),
                &EntryType::AgentPubKey
            ))
        );
        assert_eq!(
            create.entry_hash(),
            Some(&EntryHash::from_raw_36(vec![3u8; 36]))
        );

        let update = sample_action(ActionData::Update(UpdateData {
            original_action_address: ActionHash::from_raw_36(vec![6u8; 36]),
            original_entry_address: EntryHash::from_raw_36(vec![7u8; 36]),
            entry_type: EntryType::CapClaim,
            entry_hash: EntryHash::from_raw_36(vec![8u8; 36]),
        }));
        assert_eq!(update.entry_type(), Some(&EntryType::CapClaim));
        assert_eq!(
            update.entry_data(),
            Some((&EntryHash::from_raw_36(vec![8u8; 36]), &EntryType::CapClaim))
        );
    }

    #[test]
    fn action_entry_type_and_data_none_for_non_entry_actions() {
        let dna = sample_action(ActionData::Dna(DnaData {
            dna_hash: DnaHash::from_raw_36(vec![5u8; 36]),
        }));
        assert_eq!(dna.entry_type(), None);
        assert_eq!(dna.entry_data(), None);
        assert_eq!(dna.entry_hash(), None);
        assert_eq!(dna.entry_visibility(), None);

        let delete = sample_action(ActionData::Delete(DeleteData {
            deletes_address: ActionHash::from_raw_36(vec![9u8; 36]),
            deletes_entry_address: EntryHash::from_raw_36(vec![10u8; 36]),
        }));
        assert!(delete.entry_data().is_none());
    }

    #[test]
    fn action_app_entry_def_some_for_app_entry_type() {
        let app_entry_def = AppEntryDef::new(
            crate::action::EntryDefIndex(1),
            crate::action::ZomeIndex(2),
            EntryVisibility::Public,
        );
        let create = sample_action(ActionData::Create(CreateData {
            entry_type: EntryType::App(app_entry_def.clone()),
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
        }));
        assert_eq!(create.app_entry_def(), Some(&app_entry_def));
    }

    #[test]
    fn action_app_entry_def_none_for_non_app_entry_type() {
        let create = sample_action(sample_create_data());
        assert_eq!(create.app_entry_def(), None);

        let dna = sample_action(ActionData::Dna(DnaData {
            dna_hash: DnaHash::from_raw_36(vec![5u8; 36]),
        }));
        assert_eq!(dna.app_entry_def(), None);
    }

    #[test]
    fn action_into_entry_data_moves_the_fields_out() {
        let create = sample_action(sample_create_data());
        let (hash, ty) = create.into_entry_data().expect("create has entry data");
        assert_eq!(hash, EntryHash::from_raw_36(vec![3u8; 36]));
        assert_eq!(ty, EntryType::AgentPubKey);

        let dna = sample_action(ActionData::Dna(DnaData {
            dna_hash: DnaHash::from_raw_36(vec![5u8; 36]),
        }));
        assert!(dna.into_entry_data().is_none());
    }

    #[test]
    fn action_entry_visibility_reads_through_entry_type() {
        let create = sample_action(sample_create_data());
        assert_eq!(create.entry_visibility(), Some(&EntryVisibility::Public));

        let cap_claim = sample_action(ActionData::Create(CreateData {
            entry_type: EntryType::CapClaim,
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
        }));
        assert_eq!(
            cap_claim.entry_visibility(),
            Some(&EntryVisibility::Private)
        );
    }

    #[test]
    fn action_is_genesis_below_threshold() {
        let mut a = sample_action(sample_create_data());
        a.header.action_seq = 0;
        assert!(a.is_genesis());
        a.header.action_seq = crate::action::POST_GENESIS_SEQ_THRESHOLD;
        assert!(!a.is_genesis());
    }

    #[test]
    fn action_signer_defaults_to_author() {
        let a = sample_action(sample_create_data());
        assert_eq!(a.signer(), a.author());
    }

    #[test]
    fn action_signer_uses_the_migration_agent_for_close_chain() {
        let new_agent = AgentPubKey::from_raw_36(vec![7u8; 36]);
        let a = sample_action(ActionData::CloseChain(CloseChainData {
            new_target: Some(crate::action::MigrationTarget::Agent(new_agent.clone())),
        }));
        assert_eq!(a.signer(), &new_agent);
        assert_ne!(a.signer(), a.author());
    }

    #[test]
    fn action_signer_uses_author_for_close_chain_without_agent_target() {
        let a = sample_action(ActionData::CloseChain(CloseChainData { new_target: None }));
        assert_eq!(a.signer(), a.author());
    }
}

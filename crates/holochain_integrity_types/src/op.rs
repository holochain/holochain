//! # Dht Operational Transforms

use crate::{
    Action, ActionRef, ActionType, AppEntryType, Create, CreateLink, Delete, DeleteLink, Entry,
    EntryType, LinkTag, MembraneProof, Record, SignedActionHashed, SignedHashed, UnitEnum, Update,
};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash, HashableContent};
use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// These are the operational transformations that can be applied to Holochain data.
/// Every [`Action`] produces a set of operations.
/// These operations are each sent to an authority for validation.
///
/// ## Producing Operations
/// The following is a list of the operations that can be produced by each [`Action`]:
/// - Every [`Action`] produces a [`Op::RegisterAgentActivity`] and a [`Op::StoreRecord`].
/// - [`Action::Create`] also produces a [`Op::StoreEntry`].
/// - [`Action::Update`] also produces a [`Op::StoreEntry`] and a [`Op::RegisterUpdate`].
/// - [`Action::Delete`] also produces a [`Op::RegisterDelete`].
/// - [`Action::CreateLink`] also produces a [`Op::RegisterCreateLink`].
/// - [`Action::DeleteLink`] also produces a [`Op::RegisterDeleteLink`].
///
/// ## Authorities
/// There are three types of authorities in Holochain:
///
/// #### The Action Authority
/// This set of authorities receives the [`Op::StoreRecord`].
/// This is where you can implement your own logic for checking
/// that it is valid to store any of the [`Action`] variants
/// according to your own applications rules.
///
/// #### The Entry Authority
/// This set of authorities receives the [`Op::StoreEntry`].
/// This is where you can implement your own logic for checking
/// that it is valid to store an [`Entry`].
/// You can think of this as the "Create" from the CRUD acronym.
///
/// ##### Metadata
/// The entry authority is also responsible for storing the metadata for each entry.
/// They receive the [`Op::RegisterUpdate`] and [`Op::RegisterDelete`].
/// This is where you can implement your own logic for checking that it is valid to
/// update or delete any of the [`Entry`] types defined in your application.
/// You can think of this as the "Update" and "Delete" from the CRUD acronym.
///
/// They receive the [`Op::RegisterCreateLink`] and [`Op::RegisterDeleteLink`].
/// This is where you can implement your own logic for checking that it is valid to
/// place a link on a base [`Entry`].
///
/// #### The Chain Authority
/// This set of authorities receives the [`Op::RegisterAgentActivity`].
/// This is where you can implement your own logic for checking that it is valid to
/// add a new [`Action`] to an agent source chain.
/// You are not validating the individual record but the entire agents source chain.
///
/// ##### Author
/// When authoring a new [`Action`] to your source chain, the
/// validation will be run from the perspective of every authority.
///
/// ##### A note on metadata for the Action authority.
/// Technically speaking the Action authority also receives and validates the
/// [`Op::RegisterUpdate`] and [`Op::RegisterDelete`] but they run the same callback
/// as the Entry authority because it would be inconsistent to have two separate
/// validation outcomes for these ops.
///
/// ## Running Validation
/// When the `fn validate(op: Op) -> ExternResult<ValidateCallbackResult>` is called
/// it will be passed the operation variant for the authority that is
/// actually running the validation.
///
/// For example the entry authority will be passed the [`Op::StoreEntry`] operation.
/// The operational transforms that can are applied to Holochain data.
/// Operations beginning with `Store` are concerned with creating and
/// storing data.
/// Operations beginning with `Register` are concerned with registering
/// metadata about the data.
pub enum Op {
    /// Stores a new [`Record`] in the DHT.
    /// This is the act of creating a new [`Action`]
    /// and publishing it to the DHT.
    /// Note that not all [`Action`]s contain an [`Entry`].
    StoreRecord(StoreRecord),
    /// Stores a new [`Entry`] in the DHT.
    /// This is the act of creating a either a [`Action::Create`] or
    /// a [`Action::Update`] and publishing it to the DHT.
    /// These actions create a new instance of an [`Entry`].
    StoreEntry(StoreEntry),
    /// Registers an update from an instance of an [`Entry`] in the DHT.
    /// This is the act of creating a [`Action::Update`] and
    /// publishing it to the DHT.
    /// Note that the [`Action::Update`] stores an new instance
    /// of an [`Entry`] and registers it as an update to the original [`Entry`].
    /// This operation is only concerned with registering the update.
    RegisterUpdate(RegisterUpdate),
    /// Registers a deletion of an instance of an [`Entry`] in the DHT.
    /// This is the act of creating a [`Action::Delete`] and
    /// publishing it to the DHT.
    RegisterDelete(RegisterDelete),
    /// Registers a new [`Action`] on an agent source chain.
    /// This is the act of creating any [`Action`] and
    /// publishing it to the DHT.
    RegisterAgentActivity(RegisterAgentActivity),
    /// Registers a link between two [`Entry`]s.
    /// This is the act of creating a [`Action::CreateLink`] and
    /// publishing it to the DHT.
    /// The authority is the entry authority for the base [`Entry`].
    RegisterCreateLink(RegisterCreateLink),
    /// Deletes a link between two [`Entry`]s.
    /// This is the act of creating a [`Action::DeleteLink`] and
    /// publishing it to the DHT.
    /// The delete always references a specific [`Action::CreateLink`].
    RegisterDeleteLink(RegisterDeleteLink),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Stores a new [`Record`] in the DHT.
/// This is the act of creating a new [`Action`]
/// and publishing it to the DHT.
/// Note that not all [`Action`]s contain an [`Entry`].
pub struct StoreRecord {
    /// The [`Record`] to store.
    pub record: Record,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Stores a new [`Entry`] in the DHT.
/// This is the act of creating a either a [`Action::Create`] or
/// a [`Action::Update`] and publishing it to the DHT.
/// These actions create a new instance of an [`Entry`].
pub struct StoreEntry {
    /// The signed and hashed [`EntryCreationAction`] that creates
    /// a new instance of the [`Entry`].
    pub action: SignedHashed<EntryCreationAction>,
    /// The new [`Entry`] to store.
    pub entry: Entry,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Registers an update from an instance of an [`Entry`] in the DHT.
/// This is the act of creating a [`Action::Update`] and
/// publishing it to the DHT.
/// Note that the [`Action::Update`] stores an new instance
/// of an [`Entry`] and registers it as an update to the original [`Entry`].
/// This operation is only concerned with registering the update.
pub struct RegisterUpdate {
    /// The signed and hashed [`Action::Update`] that registers the update.
    pub update: SignedHashed<Update>,
    /// The new [`Entry`] that is being updated to.
    pub new_entry: Option<Entry>,
    /// The original [`EntryCreationAction`] that created
    /// the original [`Entry`].
    /// Note that the update points to a specific instance of the
    /// of the original [`Entry`].
    pub original_action: EntryCreationAction,
    /// The original [`Entry`] that is being updated from.
    pub original_entry: Option<Entry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Registers a deletion of an instance of an [`Entry`] in the DHT.
/// This is the act of creating a [`Action::Delete`] and
/// publishing it to the DHT.
pub struct RegisterDelete {
    /// The signed and hashed [`Action::Delete`] that registers the deletion.
    pub delete: SignedHashed<Delete>,
    /// The original [`EntryCreationAction`] that created
    /// the original [`Entry`].
    pub original_action: EntryCreationAction,
    /// The original [`Entry`] that is being deleted.
    pub original_entry: Option<Entry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Registers a new [`Action`] on an agent source chain.
/// This is the act of creating any [`Action`] and
/// publishing it to the DHT.
pub struct RegisterAgentActivity {
    /// The signed and hashed [`Action`] that is being registered.
    pub action: SignedActionHashed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Registers a link between two [`Entry`]s.
/// This is the act of creating a [`Action::CreateLink`] and
/// publishing it to the DHT.
/// The authority is the entry authority for the base [`Entry`].
pub struct RegisterCreateLink {
    /// The signed and hashed [`Action::CreateLink`] that registers the link.
    pub create_link: SignedHashed<CreateLink>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Deletes a link between two [`Entry`]s.
/// This is the act of creating a [`Action::DeleteLink`] and
/// publishing it to the DHT.
/// The delete always references a specific [`Action::CreateLink`].
pub struct RegisterDeleteLink {
    /// The signed and hashed [`Action::DeleteLink`] that registers the deletion.
    pub delete_link: SignedHashed<DeleteLink>,
    /// The link that is being deleted.
    pub create_link: CreateLink,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Either a [`Action::Create`] or a [`Action::Update`].
/// These actions both create a new instance of an [`Entry`].
pub enum EntryCreationAction {
    /// A [`Action::Create`] that creates a new instance of an [`Entry`].
    Create(Create),
    /// A [`Action::Update`] that creates a new instance of an [`Entry`].
    Update(Update),
}

impl Op {
    /// Get the [`AgentPubKey`] for the author of this op.
    pub fn author(&self) -> &AgentPubKey {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().author(),
            Op::StoreEntry(StoreEntry { action, .. }) => action.hashed.author(),
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => &update.hashed.author,
            Op::RegisterDelete(RegisterDelete { delete, .. }) => &delete.hashed.author,
            Op::RegisterAgentActivity(RegisterAgentActivity { action }) => action.hashed.author(),
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                &create_link.hashed.author
            }
            Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => {
                &delete_link.hashed.author
            }
        }
    }
    /// Get the [`Timestamp`] for when this op was created.
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().timestamp(),
            Op::StoreEntry(StoreEntry { action, .. }) => *action.hashed.timestamp(),
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => update.hashed.timestamp,
            Op::RegisterDelete(RegisterDelete { delete, .. }) => delete.hashed.timestamp,
            Op::RegisterAgentActivity(RegisterAgentActivity { action }) => {
                action.hashed.timestamp()
            }
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                create_link.hashed.timestamp
            }
            Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => {
                delete_link.hashed.timestamp
            }
        }
    }
    /// Get the action sequence this op.
    pub fn action_seq(&self) -> u32 {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().action_seq(),
            Op::StoreEntry(StoreEntry { action, .. }) => *action.hashed.action_seq(),
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => update.hashed.action_seq,
            Op::RegisterDelete(RegisterDelete { delete, .. }) => delete.hashed.action_seq,
            Op::RegisterAgentActivity(RegisterAgentActivity { action }) => {
                action.hashed.action_seq()
            }
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                create_link.hashed.action_seq
            }
            Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => {
                delete_link.hashed.action_seq
            }
        }
    }
    /// Get the [`ActionHash`] for the the previous action from this op if there is one.
    pub fn prev_action(&self) -> Option<&ActionHash> {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().prev_action(),
            Op::StoreEntry(StoreEntry { action, .. }) => Some(action.hashed.prev_action()),
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => Some(&update.hashed.prev_action),
            Op::RegisterDelete(RegisterDelete { delete, .. }) => Some(&delete.hashed.prev_action),
            Op::RegisterAgentActivity(RegisterAgentActivity { action }) => {
                action.hashed.prev_action()
            }
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                Some(&create_link.hashed.prev_action)
            }
            Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => {
                Some(&delete_link.hashed.prev_action)
            }
        }
    }
    /// Get the [`ActionType`] of this op.
    pub fn action_type(&self) -> ActionType {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().action_type(),
            Op::StoreEntry(StoreEntry { action, .. }) => action.hashed.action_type(),
            Op::RegisterUpdate(RegisterUpdate { .. }) => ActionType::Update,
            Op::RegisterDelete(RegisterDelete { .. }) => ActionType::Delete,
            Op::RegisterAgentActivity(RegisterAgentActivity { action }) => {
                action.hashed.action_type()
            }
            Op::RegisterCreateLink(RegisterCreateLink { .. }) => ActionType::CreateLink,
            Op::RegisterDeleteLink(RegisterDeleteLink { .. }) => ActionType::DeleteLink,
        }
    }
}

impl EntryCreationAction {
    /// The author of this action.
    pub fn author(&self) -> &AgentPubKey {
        match self {
            EntryCreationAction::Create(Create { author, .. })
            | EntryCreationAction::Update(Update { author, .. }) => author,
        }
    }
    /// The [`Timestamp`] for this action.
    pub fn timestamp(&self) -> &Timestamp {
        match self {
            EntryCreationAction::Create(Create { timestamp, .. })
            | EntryCreationAction::Update(Update { timestamp, .. }) => timestamp,
        }
    }
    /// The action sequence number of this action.
    pub fn action_seq(&self) -> &u32 {
        match self {
            EntryCreationAction::Create(Create { action_seq, .. })
            | EntryCreationAction::Update(Update { action_seq, .. }) => action_seq,
        }
    }
    /// The previous [`ActionHash`] of the previous action in the source chain.
    pub fn prev_action(&self) -> &ActionHash {
        match self {
            EntryCreationAction::Create(Create { prev_action, .. })
            | EntryCreationAction::Update(Update { prev_action, .. }) => prev_action,
        }
    }
    /// The [`EntryType`] of the [`Entry`] being created.
    pub fn entry_type(&self) -> &EntryType {
        match self {
            EntryCreationAction::Create(Create { entry_type, .. })
            | EntryCreationAction::Update(Update { entry_type, .. }) => entry_type,
        }
    }
    /// The [`EntryHash`] of the [`Entry`] being created.
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            EntryCreationAction::Create(Create { entry_hash, .. })
            | EntryCreationAction::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }
    /// The [`AppEntryType`] of the [`Entry`] being created if it
    /// is an application defined [`Entry`].
    pub fn app_entry_type(&self) -> Option<&AppEntryType> {
        match self.entry_type() {
            EntryType::App(app_entry_type) => Some(app_entry_type),
            _ => None,
        }
    }

    /// Returns `true` if this action creates an [`EntryType::AgentPubKey`] [`Entry`].
    pub fn is_agent_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::AgentPubKey)
    }

    /// Returns `true` if this action creates an [`EntryType::CapClaim`] [`Entry`].
    pub fn is_cap_claim_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::CapClaim)
    }

    /// Returns `true` if this action creates an [`EntryType::CapGrant`] [`Entry`].
    pub fn is_cap_grant_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::CapGrant)
    }

    /// Get the [`ActionType`] for this.
    pub fn action_type(&self) -> ActionType {
        match self {
            EntryCreationAction::Create(_) => ActionType::Create,
            EntryCreationAction::Update(_) => ActionType::Update,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A convenience type for validation [`Op`]s.
pub enum OpType<ET, LT>
where
    ET: UnitEnum,
{
    /// The [`Op::StoreRecord`] which is validated by the authority
    /// for the [`ActionHash`] of this record.
    ///
    /// This operation stores a [`Record`] on the DHT and is
    /// returned when the authority receives a request
    /// on the [`ActionHash`].
    StoreRecord(OpRecord<ET, LT>),
    /// The [`Op::StoreEntry`] which is validated by the authority
    /// for the [`EntryHash`] of this entry.
    ///
    /// This operation stores an [`Entry`] on the DHT and is
    /// returned when the authority receives a request
    /// on the [`EntryHash`].
    StoreEntry(OpEntry<ET>),
    /// The [`Op::RegisterAgentActivity`] which is validated by
    /// the authority for the [`AgentPubKey`] for the author of this [`Action`].
    ///
    /// This operation registers an [`Action`] to an agent's chain
    /// on the DHT and is returned when the authority receives a request
    /// on the [`AgentPubKey`] for chain data.
    ///
    /// Note that [`Op::RegisterAgentActivity`] is the only operation
    /// that is validated by all zomes regardless of entry or link types.
    RegisterAgentActivity(OpActivity<<ET as UnitEnum>::Unit, LT>),
    /// The [`Op::RegisterCreateLink`] which is validated by
    /// the authority for the [`AnyLinkableHash`] in the base address
    /// of this link.
    ///
    /// This operation register's a link to the base address
    /// on the DHT and is returned when the authority receives a request
    /// on the base [`AnyLinkableHash`] for links.
    RegisterCreateLink {
        /// The base address where this link is stored.
        base_address: AnyLinkableHash,
        /// The target address of this link.
        target_address: AnyLinkableHash,
        /// The link's tag data.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
    },
    /// The [`Op::RegisterDeleteLink`] which is validated by
    /// the authority for the [`AnyLinkableHash`] in the base address
    /// of the link that is being deleted.
    ///
    /// This operation registers a deletion of a link to the base address
    /// on the DHT and is returned when the authority receives a request
    /// on the base [`AnyLinkableHash`] for the link that is being deleted.
    RegisterDeleteLink {
        /// The hash deleted links [`Action`].
        original_link_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The target address of the link being deleted.
        target_address: AnyLinkableHash,
        /// The deleted links tag data.
        tag: LinkTag,
        /// The app defined link type of the deleted link.
        link_type: LT,
    },
    /// The [`Op::RegisterUpdate`] which is validated by
    /// the authority for the [`ActionHash`] of the original entry
    /// and the authority for the [`EntryHash`] of the original entry.
    ///
    /// This operation registers an update from the original entry on
    /// the DHT and is returned when the authority receives a request
    /// for the [`ActionHash`] of the original entry [`Action`] or the
    /// [`EntryHash`] of the original entry.
    RegisterUpdate(OpUpdate<ET>),
    /// The [`Op::RegisterDelete`] which is validated by
    /// the authority for the [`ActionHash`] of the deleted entry
    /// and the authority for the [`EntryHash`] of the deleted entry.
    ///
    /// This operation registers a deletion to the original entry on
    /// the DHT and is returned when the authority receives a request
    /// for the [`ActionHash`] of the deleted entry [`Action`] or the
    /// [`EntryHash`] of the deleted entry.
    RegisterDelete(OpDelete<ET>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreRecord`] operation.
pub enum OpRecord<ET, LT>
where
    ET: UnitEnum,
{
    /// This operation stores the [`Record`] for an
    /// app defined entry type.
    CreateEntry {
        /// The hash of the [`Entry`] being created.
        entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data.
        entry_type: ET,
    },
    /// This operation stores the [`Record`] for an
    /// app defined private entry type.
    CreatePrivateEntry {
        /// The hash of the [`Entry`] being created.
        entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// Note it is not possible to deserialize the full
        /// entry type here because we don't have the [`Entry`] data.
        entry_type: <ET as UnitEnum>::Unit,
    },
    /// This operation stores the [`Record`] for an
    /// [`AgentPubKey`] that has been created.
    CreateAgent(AgentPubKey),
    /// This operation stores the [`Record`] for a
    /// Capability Claim that has been created.
    CreateCapClaim(EntryHash),
    /// This operation stores the [`Record`] for a
    /// Capability Grant that has been created.
    CreateCapGrant(EntryHash),
    /// This operation stores the [`Record`] for an
    /// updated app defined entry type.
    UpdateEntry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data from the new entry.
        /// Note the new entry type is always the same as the
        /// original entry type however the data may have changed.
        entry_type: ET,
    },
    /// This operation stores the [`Record`] for an
    /// updated app defined private entry type.
    UpdatePrivateEntry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// Note the new entry type is always the same as the
        /// original entry type however the data may have changed.
        entry_type: <ET as UnitEnum>::Unit,
    },
    /// This operation stores the [`Record`] for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`].
        original_action_hash: ActionHash,
    },
    /// This operation stores the [`Record`] for an
    /// updated Capability Claim.
    UpdateCapClaim {
        /// The hash of the newly created Capability Claim.
        entry_hash: EntryHash,
        /// The hash of the original Capability Claim's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original Capability Claim.
        original_entry_hash: EntryHash,
    },
    /// This operation stores the [`Record`] for an
    /// updated Capability Grant.
    UpdateCapGrant {
        /// The hash of the newly created Capability Grant.
        entry_hash: EntryHash,
        /// The hash of the original Capability Grant's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original Capability Grant.
        original_entry_hash: EntryHash,
    },
    /// This operation stores the [`Record`] for a
    /// deleted app defined entry type.
    DeleteEntry {
        /// The hash of the deleted entry's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the deleted entry.
        original_entry_hash: EntryHash,
    },
    /// This operation stores the [`Record`] for a
    /// new link.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
    },
    /// This operation stores the [`Record`] for a
    /// deleted link and contains the original link's
    /// [`Action`] hash.
    DeleteLink(ActionHash),
    /// This operation stores the [`Record`] for an
    /// [`Action::Dna`].
    Dna(DnaHash),
    /// This operation stores the [`Record`] for an
    /// [`Action::OpenChain`] and contains the previous
    /// chains's [`DnaHash`].
    OpenChain(DnaHash),
    /// This operation stores the [`Record`] for an
    /// [`Action::CloseChain`] and contains the new
    /// chains's [`DnaHash`].
    CloseChain(DnaHash),
    /// This operation stores the [`Record`] for an
    /// [`Action::AgentValidationPackage`] and contains
    /// the membrane proof ig there is one.
    AgentValidationPkg(Option<MembraneProof>),
    /// This operation stores the [`Record`] for an
    /// [`Action::InitZomesComplete`].
    InitZomesComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterAgentActivity`] operation.
pub enum OpActivity<UnitType, LT> {
    /// This operation registers the [`Action`] for an
    /// app defined entry type to the author's chain.
    CreateEntry {
        /// The hash of the [`Entry`] being created.
        entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        entry_type: Option<UnitType>,
    },
    /// This operation registers the [`Action`] for an
    /// app defined private entry type to the author's chain.
    CreatePrivateEntry {
        /// The hash of the [`Entry`] being created.
        entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        entry_type: Option<UnitType>,
    },
    /// This operation registers the [`Action`] for an
    /// [`AgentPubKey`] to the author's chain.
    CreateAgent(AgentPubKey),
    /// This operation registers the [`Action`] for a
    /// Capability Claim to the author's chain.
    CreateCapClaim(EntryHash),
    /// This operation registers the [`Action`] for a
    /// Capability Grant to the author's chain.
    CreateCapGrant(EntryHash),
    /// This operation registers the [`Action`] for an
    /// updated app defined entry type to the author's chain.
    UpdateEntry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        entry_type: Option<UnitType>,
    },
    /// This operation registers the [`Action`] for an
    /// updated app defined private entry type to the author's chain.
    UpdatePrivateEntry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The unit version of the app defined private entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        entry_type: Option<UnitType>,
    },
    /// This operation registers the [`Action`] for an
    /// updated [`AgentPubKey`] to the author's chain.
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`].
        original_action_hash: ActionHash,
    },
    /// This operation registers the [`Action`] for an
    /// updated Capability Claim to the author's chain.
    UpdateCapClaim {
        /// The hash of the newly created Capability Claim.
        entry_hash: EntryHash,
        /// The hash of the original Capability Claim's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original Capability Claim.
        original_entry_hash: EntryHash,
    },
    /// This operation registers the [`Action`] for an
    /// updated Capability Grant to the author's chain.
    UpdateCapGrant {
        /// The hash of the newly created Capability Grant.
        entry_hash: EntryHash,
        /// The hash of the original Capability Grant's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original Capability Grant.
        original_entry_hash: EntryHash,
    },
    /// This operation registers the [`Action`] for a
    /// deleted app defined entry type to the author's chain.
    DeleteEntry {
        /// The hash of the deleted entry's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the deleted entry.
        original_entry_hash: EntryHash,
    },
    /// This operation registers the [`Action`] for a
    /// new link to the author's chain.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        /// If this is [`None`] then the link type is defined
        /// in a different zome.
        link_type: Option<LT>,
    },
    /// This operation registers the [`Action`] for a
    /// deleted link to the author's chain and contains
    /// the original link's [`Action`] hash.
    DeleteLink(ActionHash),
    /// This operation registers the [`Action`] for an
    /// [`Action::Dna`] to the author's chain.
    Dna(DnaHash),
    /// This operation registers the [`Action`] for an
    /// [`Action::OpenChain`] to the author's chain
    /// and contains the previous chains's [`DnaHash`].
    OpenChain(DnaHash),
    /// This operation registers the [`Action`] for an
    /// [`Action::CloseChain`] to the author's chain
    /// and contains the new chains's [`DnaHash`].
    CloseChain(DnaHash),
    /// This operation registers the [`Action`] for an
    /// [`Action::AgentValidationPkg`] to the author's chain
    /// and contains the membrane proof if there is one.
    AgentValidationPkg(Option<MembraneProof>),
    /// This operation registers the [`Action`] for an
    /// [`Action::InitZomesComplete`] to the author's chain.
    InitZomesComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreEntry`] operation.
pub enum OpEntry<ET>
where
    ET: UnitEnum,
{
    /// This operation stores the [`Entry`] for an
    /// app defined entry type.
    CreateEntry {
        /// The hash of the [`Entry`] being created.
        entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data.
        entry_type: ET,
    },
    /// This operation stores the [`Entry`] for an
    /// [`AgentPubKey`].
    CreateAgent(AgentPubKey),
    /// This operation stores the [`Entry`] for the
    /// newly created entry in an update.
    UpdateEntry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the new entry.
        entry_type: ET,
    },
    /// This operation stores the [`Entry`] for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`].
        original_action_hash: ActionHash,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterUpdate`] operation.
pub enum OpUpdate<ET>
where
    ET: UnitEnum,
{
    /// This operation registers an update from
    /// the original [`Entry`].
    Entry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the original entry.
        original_entry_type: ET,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the new entry.
        new_entry_type: ET,
    },
    /// This operation registers an update from
    /// the original private [`Entry`].
    PrivateEntry {
        /// The hash of the newly created [`Entry`].
        entry_hash: EntryHash,
        /// The hash of the original [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original [`Entry`].
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type
        /// for the original entry.
        original_entry_type: <ET as UnitEnum>::Unit,
        /// The unit version of the app defined entry type
        /// for the new entry.
        new_entry_type: <ET as UnitEnum>::Unit,
    },
    /// This operation registers an update from
    /// the original [`AgentPubKey`].
    Agent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`].
        original_action_hash: ActionHash,
    },
    /// This operation registers an update from
    /// a Capability Claim.
    CapClaim {
        /// The hash of the newly created Capability Claim.
        entry_hash: EntryHash,
        /// The hash of the original Capability Claim's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original Capability Claim.
        original_entry_hash: EntryHash,
    },
    /// This operation registers an update from
    /// a Capability Grant.
    CapGrant {
        /// The hash of the newly created Capability Grant.
        entry_hash: EntryHash,
        /// The hash of the original Capability Grant's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the original Capability Grant.
        original_entry_hash: EntryHash,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterDelete`] operation.
pub enum OpDelete<ET>
where
    ET: UnitEnum,
{
    /// This operation registers a deletion to the
    /// original [`Entry`].
    Entry {
        /// The hash of the delete entry's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the delete entry.
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data from the deleted entry.
        original_entry_type: ET,
    },
    /// This operation registers a deletion to the
    /// original private [`Entry`].
    PrivateEntry {
        /// The hash of the dleted entry's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the deleted entry.
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type
        /// of the deleted entry.
        original_entry_type: <ET as UnitEnum>::Unit,
    },
    /// This operation registers a deletion to an
    /// [`AgentPubKey`].
    Agent {
        /// The deleted [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the deleted keys [`Action`].
        original_action_hash: ActionHash,
    },
    /// This operation registers a deletion to a
    /// Capability Claim.
    CapClaim {
        /// The hash of the deleted Capability Claim's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the deleted Capability Claim.
        original_entry_hash: EntryHash,
    },
    /// This operation registers a deletion to a
    /// Capability Grant.
    CapGrant {
        /// The hash of the deleted Capability Grant's [`Action`].
        original_action_hash: ActionHash,
        /// The hash of the deleted Capability Grant.
        original_entry_hash: EntryHash,
    },
}

/// Allows a [`EntryCreationAction`] to hash the same bytes as
/// the equivalent [`Action`] variant without needing to clone the action.
impl HashableContent for EntryCreationAction {
    type HashType = holo_hash::hash_type::Action;

    fn hash_type(&self) -> Self::HashType {
        use holo_hash::PrimitiveHashType;
        holo_hash::hash_type::Action::new()
    }

    fn hashable_content(&self) -> holo_hash::HashableContentBytes {
        let h = match self {
            EntryCreationAction::Create(create) => ActionRef::Create(create),
            EntryCreationAction::Update(update) => ActionRef::Update(update),
        };
        let sb = SerializedBytes::from(UnsafeBytes::from(
            holochain_serialized_bytes::encode(&h).expect("Could not serialize HashableContent"),
        ));
        holo_hash::HashableContentBytes::Content(sb)
    }
}

impl From<EntryCreationAction> for Action {
    fn from(e: EntryCreationAction) -> Self {
        match e {
            EntryCreationAction::Create(c) => Action::Create(c),
            EntryCreationAction::Update(u) => Action::Update(u),
        }
    }
}

impl From<Create> for EntryCreationAction {
    fn from(c: Create) -> Self {
        EntryCreationAction::Create(c)
    }
}

impl From<Update> for EntryCreationAction {
    fn from(u: Update) -> Self {
        EntryCreationAction::Update(u)
    }
}

impl TryFrom<Action> for EntryCreationAction {
    type Error = crate::WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::Create(h) => Ok(EntryCreationAction::Create(h)),
            Action::Update(h) => Ok(EntryCreationAction::Update(h)),
            _ => Err(crate::WrongActionError(format!("{:?}", value))),
        }
    }
}

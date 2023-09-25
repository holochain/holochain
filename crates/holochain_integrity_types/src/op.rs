//! # Dht Operations

use crate::{
    Action, ActionRef, ActionType, AppEntryDef, Create, CreateLink, Delete, DeleteLink, Entry,
    EntryType, Record, SignedActionHashed, SignedHashed, Update,
};
use holo_hash::{ActionHash, AgentPubKey, EntryHash, HashableContent};
use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// These are the operations that can be applied to Holochain data.
/// Every [`Action`] produces a set of operations.
/// These operations are each sent to an authority for validation.
///
/// # Examples
///
/// Validate a new entry: <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/validate/src/integrity.rs>
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
/// The operations that can be applied to Holochain data.
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
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
    /// This will be [`None`] when the [`Entry`] being
    /// created is [`EntryVisibility::Private`](crate::entry_def::EntryVisibility::Private).
    pub new_entry: Option<Entry>,
    /// The original [`EntryCreationAction`] that created
    /// the original [`Entry`].
    /// Note that the update points to a specific instance of the
    /// of the original [`Entry`].
    pub original_action: EntryCreationAction,
    /// The original [`Entry`] that is being updated from.
    /// This will be [`None`] when the [`Entry`] being
    /// updated is [`EntryVisibility::Private`](crate::entry_def::EntryVisibility::Private).
    pub original_entry: Option<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
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
    /// This will be [`None`] when the [`Entry`] being
    /// deleted is [`EntryVisibility::Private`](crate::entry_def::EntryVisibility::Private).
    pub original_entry: Option<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Registers a new [`Action`] on an agent source chain.
/// This is the act of creating any [`Action`] and
/// publishing it to the DHT.
pub struct RegisterAgentActivity {
    /// The signed and hashed [`Action`] that is being registered.
    pub action: SignedActionHashed,
    /// Entries can be cached with agent authorities if
    /// `cached_at_agent_activity` is set to true for an entries
    /// definitions.
    /// If it is cached for this action then this will be some.
    pub cached_entry: Option<Entry>,
}

impl AsRef<SignedActionHashed> for RegisterAgentActivity {
    fn as_ref(&self) -> &SignedActionHashed {
        &self.action
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Registers a link between two [`Entry`]s.
/// This is the act of creating a [`Action::CreateLink`] and
/// publishing it to the DHT.
/// The authority is the entry authority for the base [`Entry`].
pub struct RegisterCreateLink {
    /// The signed and hashed [`Action::CreateLink`] that registers the link.
    pub create_link: SignedHashed<CreateLink>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
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

impl Op {
    /// Get the [`AgentPubKey`] for the author of this op.
    pub fn author(&self) -> &AgentPubKey {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().author(),
            Op::StoreEntry(StoreEntry { action, .. }) => action.hashed.author(),
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => &update.hashed.author,
            Op::RegisterDelete(RegisterDelete { delete, .. }) => &delete.hashed.author,
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
                action.hashed.author()
            }
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
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
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
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
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
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
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
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
                action.hashed.action_type()
            }
            Op::RegisterCreateLink(RegisterCreateLink { .. }) => ActionType::CreateLink,
            Op::RegisterDeleteLink(RegisterDeleteLink { .. }) => ActionType::DeleteLink,
        }
    }

    /// Get the entry-related data for this op, if applicable
    pub fn entry_data(&self) -> Option<(&EntryHash, &EntryType)> {
        match self {
            Op::StoreRecord(StoreRecord { record }) => record.action().entry_data(),
            Op::StoreEntry(StoreEntry { action, .. }) => {
                Some((action.hashed.entry_hash(), action.hashed.entry_type()))
            }
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => {
                Some((&update.hashed.entry_hash, &update.hashed.entry_type))
            }
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
                action.hashed.entry_data()
            }
            Op::RegisterDelete(_) | Op::RegisterCreateLink(_) | Op::RegisterDeleteLink(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes, Eq)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Either a [`Action::Create`] or a [`Action::Update`].
/// These actions both create a new instance of an [`Entry`].
pub enum EntryCreationAction {
    /// A [`Action::Create`] that creates a new instance of an [`Entry`].
    Create(Create),
    /// A [`Action::Update`] that creates a new instance of an [`Entry`].
    Update(Update),
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
    /// The [`AppEntryDef`] of the [`Entry`] being created if it
    /// is an application defined [`Entry`].
    pub fn app_entry_def(&self) -> Option<&AppEntryDef> {
        match self.entry_type() {
            EntryType::App(app_entry_def) => Some(app_entry_def),
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

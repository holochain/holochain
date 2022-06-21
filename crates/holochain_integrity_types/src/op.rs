//! # Dht Operational Transforms

use crate::{
    Action, ActionRef, AppEntryType, Commit, Create, CreateLink, Delete, DeleteLink, Entry,
    EntryType, SignedActionHashed, SignedHashed, Update,
};
use holo_hash::{ActionHash, AgentPubKey, EntryHash, HashableContent};
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
/// - Every [`Action`] produces a [`Op::RegisterAgentActivity`] and a [`Op::StoreCommit`].
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
/// This set of authorities receives the [`Op::StoreCommit`].
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
/// You are not validating the individual commit but the entire agents source chain.
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
    /// Stores a new [`Commit`] in the DHT.
    /// This is the act of creating a new [`Action`]
    /// and publishing it to the DHT.
    /// Note that not all [`Action`]s contain an [`Entry`].
    StoreCommit {
        /// The [`Commit`] to store.
        commit: Commit,
    },
    /// Stores a new [`Entry`] in the DHT.
    /// This is the act of creating a either a [`Action::Create`] or
    /// a [`Action::Update`] and publishing it to the DHT.
    /// These actions create a new instance of an [`Entry`].
    StoreEntry {
        /// The signed and hashed [`EntryCreationAction`] that creates
        /// a new instance of the [`Entry`].
        action: SignedHashed<EntryCreationAction>,
        /// The new [`Entry`] to store.
        entry: Entry,
    },
    /// Registers an update from an instance of an [`Entry`] in the DHT.
    /// This is the act of creating a [`Action::Update`] and
    /// publishing it to the DHT.
    /// Note that the [`Action::Update`] stores an new instance
    /// of an [`Entry`] and registers it as an update to the original [`Entry`].
    /// This operation is only concerned with registering the update.
    RegisterUpdate {
        /// The signed and hashed [`Action::Update`] that registers the update.
        update: SignedHashed<Update>,
        /// The new [`Entry`] that is being updated to.
        new_entry: Entry,
        /// The original [`EntryCreationAction`] that created
        /// the original [`Entry`].
        /// Note that the update points to a specific instance of the
        /// of the original [`Entry`].
        original_action: EntryCreationAction,
        /// The original [`Entry`] that is being updated from.
        original_entry: Entry,
    },
    /// Registers a deletion of an instance of an [`Entry`] in the DHT.
    /// This is the act of creating a [`Action::Delete`] and
    /// publishing it to the DHT.
    RegisterDelete {
        /// The signed and hashed [`Action::Delete`] that registers the deletion.
        delete: SignedHashed<Delete>,
        /// The original [`EntryCreationAction`] that created
        /// the original [`Entry`].
        original_action: EntryCreationAction,
        /// The original [`Entry`] that is being deleted.
        original_entry: Entry,
    },
    /// Registers a new [`Action`] on an agent source chain.
    /// This is the act of creating any [`Action`] and
    /// publishing it to the DHT.
    RegisterAgentActivity {
        /// The signed and hashed [`Action`] that is being registered.
        action: SignedActionHashed,
    },
    /// Registers a link between two [`Entry`]s.
    /// This is the act of creating a [`Action::CreateLink`] and
    /// publishing it to the DHT.
    /// The authority is the entry authority for the base [`Entry`].
    RegisterCreateLink {
        /// The signed and hashed [`Action::CreateLink`] that registers the link.
        create_link: SignedHashed<CreateLink>,
    },
    /// Deletes a link between two [`Entry`]s.
    /// This is the act of creating a [`Action::DeleteLink`] and
    /// publishing it to the DHT.
    /// The delete always references a specific [`Action::CreateLink`].
    RegisterDeleteLink {
        /// The signed and hashed [`Action::DeleteLink`] that registers the deletion.
        delete_link: SignedHashed<DeleteLink>,
        /// The link that is being deleted.
        create_link: CreateLink,
    },
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

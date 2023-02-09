use crate::{
    Action, ActionRef, ActionType, AgentValidationPkg, AppEntryDef, CloseChain, Create, CreateLink,
    Delete, DeleteLink, Dna, Entry, EntryType, InitZomesComplete, LinkTag, MembraneProof,
    OpenChain, Record, SignedActionHashed, SignedHashed, UnitEnum, Update,
};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash, HashableContent};
use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

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

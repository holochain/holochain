//! # Dht Operations

use crate::{ActionType, AppEntryDef, Create, EntryType, Update};
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;

/// Either a [`Create`] or a [`Update`].
/// These actions both create a new instance of an [`Entry`](crate::Entry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes, Eq)]
pub enum EntryCreationAction {
    /// A [`Create`] that creates a new instance of an [`Entry`](crate::Entry).
    Create(Create),
    /// A [`Update`] that creates a new instance of an [`Entry`](crate::Entry).
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
    /// The [`EntryType`] of the [`Entry`](crate::Entry) being created.
    pub fn entry_type(&self) -> &EntryType {
        match self {
            EntryCreationAction::Create(Create { entry_type, .. })
            | EntryCreationAction::Update(Update { entry_type, .. }) => entry_type,
        }
    }
    /// The [`EntryHash`] of the [`Entry`](crate::Entry) being created.
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            EntryCreationAction::Create(Create { entry_hash, .. })
            | EntryCreationAction::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }
    /// The [`AppEntryDef`] of the [`Entry`](crate::Entry) being created if it
    /// is an application defined [`Entry`](crate::Entry).
    pub fn app_entry_def(&self) -> Option<&AppEntryDef> {
        match self.entry_type() {
            EntryType::App(app_entry_def) => Some(app_entry_def),
            _ => None,
        }
    }

    /// Returns `true` if this action creates an [`EntryType::AgentPubKey`] [`Entry`](crate::Entry).
    pub fn is_agent_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::AgentPubKey)
    }

    /// Returns `true` if this action creates an [`EntryType::CapClaim`] [`Entry`](crate::Entry).
    pub fn is_cap_claim_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::CapClaim)
    }

    /// Returns `true` if this action creates an [`EntryType::CapGrant`] [`Entry`](crate::Entry).
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

/// Allows a [`EntryCreationAction`] to hash the same bytes as the equivalent
/// [`Action`](crate::dht_v2::Action) variant, hashing the
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
/// A utility trait for associating a data enum
/// with a unit enum that has the same variants.
pub trait UnitEnum {
    /// An enum with the same variants as the implementor
    /// but without any data.
    type Unit: core::fmt::Debug
        + Clone
        + Copy
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + core::hash::Hash;

    /// Turn this type into it's unit enum.
    fn to_unit(&self) -> Self::Unit;

    /// Iterate over the unit variants.
    fn unit_iter() -> Box<dyn Iterator<Item = Self::Unit>>;
}

/// Needed as a base case for ignoring types.
impl UnitEnum for () {
    type Unit = ();

    fn to_unit(&self) -> Self::Unit {}

    fn unit_iter() -> Box<dyn Iterator<Item = Self::Unit>> {
        Box::new([].into_iter())
    }
}

/// A full UnitEnum, or just the unit type of that UnitEnum
#[derive(Clone, Debug)]
pub enum UnitEnumEither<E: UnitEnum> {
    /// The full enum
    Enum(E),
    /// Just the unit enum
    Unit(E::Unit),
}

//! Holochain's [`Action`] and its variations.
//!
//! All action variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first action
//! in a chain) contain the field `prev_action`.

#![allow(missing_docs)]

use crate::prelude::*;
use derive_more::From;
use holo_hash::EntryHash;
use holochain_zome_types::op::EntryCreationAction;

/// A action of one of the two types that create a new entry.
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash, derive_more::From,
)]
pub enum NewEntryAction {
    /// A action which simply creates a new entry
    Create(Create),
    /// A action which creates a new entry that is semantically related to a
    /// previously created entry or action
    Update(Update),
}

/// Same as NewEntryAction but takes actions as reference
#[allow(missing_docs)]
#[derive(Debug, From)]
pub enum NewEntryActionRef<'a> {
    Create(&'a Create),
    Update(&'a Update),
}

impl NewEntryAction {
    /// Get the entry on this action
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryAction::Create(Create { entry_hash, .. })
            | NewEntryAction::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }

    /// Get the entry type on this action
    pub fn entry_type(&self) -> &EntryType {
        match self {
            NewEntryAction::Create(Create { entry_type, .. })
            | NewEntryAction::Update(Update { entry_type, .. }) => entry_type,
        }
    }

    /// Get the visibility of this action
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            NewEntryAction::Create(Create { entry_type, .. })
            | NewEntryAction::Update(Update { entry_type, .. }) => entry_type.visibility(),
        }
    }

    /// Get the timestamp of this action
    pub fn timestamp(&self) -> holochain_zome_types::timestamp::Timestamp {
        match self {
            NewEntryAction::Create(Create { timestamp, .. })
            | NewEntryAction::Update(Update { timestamp, .. }) => *timestamp,
        }
    }

    /// Get the author of this action
    pub fn author(&self) -> &AgentPubKey {
        match self {
            NewEntryAction::Create(Create { author, .. })
            | NewEntryAction::Update(Update { author, .. }) => author,
        }
    }

    /// Get the action_seq of this action
    pub fn action_seq(&self) -> u32 {
        match self {
            NewEntryAction::Create(Create { action_seq, .. })
            | NewEntryAction::Update(Update { action_seq, .. }) => *action_seq,
        }
    }
}

impl From<NewEntryAction> for EntryCreationAction {
    fn from(action: NewEntryAction) -> Self {
        match action {
            NewEntryAction::Create(create) => EntryCreationAction::Create(create),
            NewEntryAction::Update(update) => EntryCreationAction::Update(update),
        }
    }
}

impl NewEntryActionRef<'_> {
    pub fn entry_type(&self) -> &EntryType {
        match self {
            NewEntryActionRef::Create(Create { entry_type, .. })
            | NewEntryActionRef::Update(Update { entry_type, .. }) => entry_type,
        }
    }
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            NewEntryActionRef::Create(Create { entry_hash, .. })
            | NewEntryActionRef::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }
    pub fn to_new_entry_action(&self) -> NewEntryAction {
        match self {
            NewEntryActionRef::Create(create) => NewEntryAction::Create((*create).to_owned()),
            NewEntryActionRef::Update(update) => NewEntryAction::Update((*update).to_owned()),
        }
    }
}

impl<'a> From<&'a NewEntryAction> for NewEntryActionRef<'a> {
    fn from(n: &'a NewEntryAction) -> Self {
        match n {
            NewEntryAction::Create(ec) => NewEntryActionRef::Create(ec),
            NewEntryAction::Update(eu) => NewEntryActionRef::Update(eu),
        }
    }
}

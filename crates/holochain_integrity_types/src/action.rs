use crate::entry_def::EntryVisibility;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;
use std::borrow::Borrow;

pub mod conversions;

/// Any action with a action_seq less than this value is part of a record
/// created during genesis. Anything with this seq or higher was created
/// after genesis.
pub const POST_GENESIS_SEQ_THRESHOLD: u32 = 3;

/// The unit enum naming the action variants. Canonically defined on the v2
/// action model and re-exported here as `holochain_integrity_types::ActionType`.
pub use crate::dht_v2::ActionType;

/// this id is an internal reference, which also serves as a canonical ordering
/// for zome initialization.  The value should be auto-generated from the Zome Bundle def
// TODO: Check this can never be written to > 255
#[derive(
    Debug,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    SerializedBytes,
)]
pub struct ZomeIndex(pub u8);

impl ZomeIndex {
    pub fn new(u: u8) -> Self {
        Self(u)
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    SerializedBytes,
)]
pub struct EntryDefIndex(pub u8);

/// Description of how to find the previous or next CellId in a migration.
/// In a migration, of the two components of the CellId (dna and agent),
/// always one stays fixed while the other one changes.
/// This enum represents the component that changed.
///
/// When used in CloseChain, this contains the new DNA hash or Agent key.
/// When used in OpenChain, this contains the previous DNA hash or Agent key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub enum MigrationTarget {
    /// Represents a DNA migration, and contains the new or previous DNA hash.
    Dna(DnaHash),
    /// Represents an Agent migration, and contains the new or previous Agent key.
    Agent(AgentPubKey),
}

impl From<DnaHash> for MigrationTarget {
    fn from(dna: DnaHash) -> Self {
        MigrationTarget::Dna(dna)
    }
}

impl From<AgentPubKey> for MigrationTarget {
    fn from(agent: AgentPubKey) -> Self {
        MigrationTarget::Agent(agent)
    }
}

/// Allows Actions which reference Entries to know what type of Entry it is
/// referencing. Useful for examining Actions without needing to fetch the
/// corresponding Entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub enum EntryType {
    /// An AgentPubKey
    AgentPubKey,
    /// An app-provided entry, along with its app-provided AppEntryDef
    App(AppEntryDef),
    /// A Capability claim
    CapClaim,
    /// A Capability grant.
    CapGrant,
}

impl EntryType {
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            EntryType::AgentPubKey => &EntryVisibility::Public,
            EntryType::App(app_entry_def) => app_entry_def.visibility(),
            EntryType::CapClaim => &EntryVisibility::Private,
            EntryType::CapGrant => &EntryVisibility::Private,
        }
    }
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryType::AgentPubKey => write!(f, "AgentPubKey"),
            EntryType::App(app_entry_def) => write!(
                f,
                "App({:?}, {:?})",
                app_entry_def.entry_index(),
                app_entry_def.visibility()
            ),
            EntryType::CapClaim => write!(f, "CapClaim"),
            EntryType::CapGrant => write!(f, "CapGrant"),
        }
    }
}

/// Information about a class of Entries provided by the DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct AppEntryDef {
    /// A unique u8 identifier within a zome for this
    /// entry type.
    pub entry_index: EntryDefIndex,
    /// The id of the zome that defines this entry type.
    pub zome_index: ZomeIndex,
    // @todo don't do this, use entry defs instead
    /// The visibility of this app entry.
    pub visibility: EntryVisibility,
}

impl AppEntryDef {
    pub fn new(
        entry_index: EntryDefIndex,
        zome_index: ZomeIndex,
        visibility: EntryVisibility,
    ) -> Self {
        Self {
            entry_index,
            zome_index,
            visibility,
        }
    }

    pub fn entry_index(&self) -> EntryDefIndex {
        self.entry_index
    }
    pub fn zome_index(&self) -> ZomeIndex {
        self.zome_index
    }
    pub fn visibility(&self) -> &EntryVisibility {
        &self.visibility
    }
}

impl From<EntryDefIndex> for u8 {
    fn from(ei: EntryDefIndex) -> Self {
        ei.0
    }
}

impl ZomeIndex {
    /// Use as an index into a slice
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

impl std::ops::Deref for ZomeIndex {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<u8> for ZomeIndex {
    fn borrow(&self) -> &u8 {
        &self.0
    }
}

pub trait ActionHashedContainer: ActionSequenceAndHash {
    fn action(&self) -> &crate::dht_v2::Action;

    fn action_hash(&self) -> &ActionHash;
}

pub trait ActionSequenceAndHash {
    fn action_seq(&self) -> u32;
    fn address(&self) -> &ActionHash;
}

impl ActionSequenceAndHash for (u32, ActionHash) {
    fn action_seq(&self) -> u32 {
        self.0
    }

    fn address(&self) -> &ActionHash {
        &self.1
    }
}

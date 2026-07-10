use crate::entry_def::EntryVisibility;
use crate::link::LinkTag;
use crate::link::LinkType;
use crate::EntryRateWeight;
use crate::MembraneProof;
use crate::RateWeight;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyLinkableHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;
use std::borrow::Borrow;
use std::hash::Hash;

pub mod builder;
pub mod conversions;

/// Any action with a action_seq less than this value is part of a record
/// created during genesis. Anything with this seq or higher was created
/// after genesis.
pub const POST_GENESIS_SEQ_THRESHOLD: u32 = 3;

/// a utility wrapper to declare the `ActionType` unit enum for our data types
macro_rules! write_into_action {
    ($($n:ident $(<$w : ty>)?),*,) => {

        /// A unit enum which just names the different action variants,
        /// without containing any extra data
        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Eq, Clone, Debug)]
        pub enum ActionType {
            $($n,)*
        }

        impl std::fmt::Display for ActionType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{}",
                    match self {
                        $( ActionType::$n => stringify!($n), )*
                    }
                )
            }
        }
    };
}

/// A trait to unify the "inner" parts of an Action, i.e. the structs inside
/// the Action enum's variants. This trait is used for the "weighed" version
/// of each struct, i.e. the version without weight information erased.
///
/// Action types with no weight are considered "weighed" and "unweighed" at the
/// same time, but types with weight have distinct types for the weighed and
/// unweighed versions.
pub trait ActionWeighed {
    type Unweighed: ActionUnweighed;
    type Weight: Default;

    /// Erase the rate limiting weight info, creating an "unweighed" version
    /// of this action. This is used primarily by validators who need to run the
    /// `weigh` callback on an action they received, and want to make sure their
    /// callback is not using the predefined weight to influence the result.
    fn unweighed(self) -> Self::Unweighed;
}

/// A trait to unify the "inner" parts of an Action, i.e. the structs inside
/// the Action enum's variants. This trait is used for the "unweighed" version
/// of each struct, i.e. the version with weight information erased.
///
/// Action types with no weight are considered "weighed" and "unweighed" at the
/// same time, but types with weight have distinct types for the weighed and
/// unweighed versions.
pub trait ActionUnweighed: Sized {
    type Weighed: ActionWeighed;
    type Weight: Default;

    /// Add a weight to this unweighed action, making it "weighed".
    /// The weight is determined by the `weigh` callback, which is run on the
    /// unweighed version of this action.
    fn weighed(self, weight: Self::Weight) -> Self::Weighed;

    /// Add zero weight to this unweighed action, making it "weighed".
    #[cfg(feature = "test_utils")]
    fn weightless(self) -> Self::Weighed {
        self.weighed(Default::default())
    }
}

write_into_action! {
    Dna,
    AgentValidationPkg,
    InitZomesComplete,
    OpenChain,
    CloseChain,

    Create<EntryRateWeight>,
    Update<EntryRateWeight>,
    Delete<RateWeight>,

    CreateLink<RateWeight>,
    DeleteLink,
}

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

/// The Dna Action is always the first action in a source chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    // No previous action, because DNA is always first chain entry
    pub hash: DnaHash,
}

/// Action for an agent validation package, used to determine whether an agent
/// is allowed to participate in this DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct AgentValidationPkg {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub membrane_proof: Option<MembraneProof>,
}

/// An action which declares that all zome init functions have successfully
/// completed, and the chain is ready for commits. Contains no explicit data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct InitZomesComplete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,
}

/// Declares that a metadata Link should be made between two hashes of anything; could be data or
/// an op or anything that can be hashed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct CreateLink<W = RateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub base_address: AnyLinkableHash,
    pub target_address: AnyLinkableHash,
    pub zome_index: ZomeIndex,
    pub link_type: LinkType,
    pub tag: LinkTag,

    pub weight: W,
}

/// Declares that a previously made Link should be nullified and considered removed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct DeleteLink {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    /// this is redundant with the `CreateLink` action but needs to be included to facilitate DHT ops
    /// this is NOT exposed to wasm developers and is validated by the subconscious to ensure that
    /// it always matches the `base_address` of the `CreateLink`
    pub base_address: AnyLinkableHash,
    /// The address of the `CreateLink` being reversed
    pub link_add_address: ActionHash,
}

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

/// When migrating to a new version of a DNA, this action is committed to the
/// old chain to declare the migration path taken. This action can also be taken
/// to simply close down a chain with no forward reference to a migration.
///
/// Note that if `MigrationTarget::Agent` is used, this action will be signed with
/// that key rather than the authoring key, so that new key must be a valid keypair
/// that you control in the keystore, so that the action can be signed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct CloseChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub new_target: Option<MigrationTarget>,
}

/// When migrating to a new version of a DNA, this action is committed to the
/// new chain to declare the migration path taken.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct OpenChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub prev_target: MigrationTarget,
    /// The hash of the `CloseChain` action on the old chain, to establish chain continuity
    /// and disallow backlinks to multiple forks on the old chain.
    pub close_hash: ActionHash,
}

/// An action which "speaks" Entry content into being. The same content can be
/// referenced by multiple such actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct Create<W = EntryRateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,

    pub weight: W,
}

/// An action which specifies that some new Entry content is intended to be an
/// update to some old Entry.
///
/// This action semantically updates an entry to a new entry.
/// It has the following effects:
/// - Create a new Entry
/// - This is the action of that new entry
/// - Create a metadata relationship between the original entry and this new action
///
/// The original action is required to prevent update loops:
/// If you update entry A to B and B back to A, and only track the original entry,
/// then you have a loop of references. Every update introduces a new action,
/// so there can only be a linear history of action updates, even if the entry history
/// experiences repeats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct Update<W = EntryRateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub original_action_address: ActionHash,
    pub original_entry_address: EntryHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,

    pub weight: W,
}

/// Declare that a previously published Action should be nullified and
/// considered deleted.
///
/// Via the associated [`crate::dht_v2::op::Op`], this also has an effect on Entries: namely,
/// that a previously published Entry will become inaccessible if all of its
/// Actions are marked deleted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct Delete<W = RateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    /// Address of the Record being deleted
    pub deletes_address: ActionHash,
    pub deletes_entry_address: EntryHash,

    pub weight: W,
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

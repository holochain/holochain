use std::borrow::Borrow;

use crate::entry_def::EntryVisibility;
use crate::link::LinkTag;
use crate::link::LinkType;
use crate::timestamp::Timestamp;
use crate::EntryRateWeight;
use crate::MembraneProof;
use crate::RateWeight;
use holo_hash::impl_hashable_content;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyLinkableHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holo_hash::HashableContent;
use holo_hash::HoloHashed;
use holochain_serialized_bytes::prelude::*;

pub mod builder;
pub mod conversions;

/// Any action with a action_seq less than this value is part of a record
/// created during genesis. Anything with this seq or higher was created
/// after genesis.
pub const POST_GENESIS_SEQ_THRESHOLD: u32 = 3;

/// Action contains variants for each type of action.
///
/// This struct really defines a local source chain, in the sense that it
/// implements the pointers between hashes that a hash chain relies on, which
/// are then used to check the integrity of data using cryptographic hash
/// functions.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[serde(tag = "type")]
pub enum Action {
    // The first action in a chain (for the DNA) doesn't have a previous action
    Dna(Dna),
    AgentValidationPkg(AgentValidationPkg),
    InitZomesComplete(InitZomesComplete),
    CreateLink(CreateLink),
    DeleteLink(DeleteLink),
    OpenChain(OpenChain),
    CloseChain(CloseChain),
    Create(Create),
    Update(Update),
    Delete(Delete),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
/// This allows action types to be serialized to bytes without requiring
/// an owned value. This produces the same bytes as if they were
/// serialized with the [`Action`] type.
pub(crate) enum ActionRef<'a> {
    Dna(&'a Dna),
    AgentValidationPkg(&'a AgentValidationPkg),
    InitZomesComplete(&'a InitZomesComplete),
    CreateLink(&'a CreateLink),
    DeleteLink(&'a DeleteLink),
    OpenChain(&'a OpenChain),
    CloseChain(&'a CloseChain),
    Create(&'a Create),
    Update(&'a Update),
    Delete(&'a Delete),
}

pub type ActionHashed = HoloHashed<Action>;

/// a utility wrapper to write intos for our data types
macro_rules! write_into_action {
    ($($n:ident $(<$w : ty>)?),*,) => {

        /// A unit enum which just maps onto the different Action variants,
        /// without containing any extra data
        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Eq, Clone, Debug)]
        #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

        impl From<&Action> for ActionType {
            fn from(action: &Action) -> ActionType {
                match action {
                    $(
                        Action::$n(_) => ActionType::$n,
                    )*
                }
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

    /// Construct the full Action enum with this variant.
    fn into_action(self) -> Action;

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

impl<I: ActionWeighed> From<I> for Action {
    fn from(i: I) -> Self {
        i.into_action()
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

/// a utility macro just to not have to type in the match statement everywhere.
macro_rules! match_action {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            Action::Dna($i) => { $($t)* }
            Action::AgentValidationPkg($i) => { $($t)* }
            Action::InitZomesComplete($i) => { $($t)* }
            Action::CreateLink($i) => { $($t)* }
            Action::DeleteLink($i) => { $($t)* }
            Action::OpenChain($i) => { $($t)* }
            Action::CloseChain($i) => { $($t)* }
            Action::Create($i) => { $($t)* }
            Action::Update($i) => { $($t)* }
            Action::Delete($i) => { $($t)* }
        }
    };
}

impl Action {
    /// Returns the address and entry type of the Entry, if applicable.
    // TODO: DRY: possibly create an `EntryData` struct which is used by both
    // Create and Update
    pub fn entry_data(&self) -> Option<(&EntryHash, &EntryType)> {
        match self {
            Self::Create(Create {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::Update(Update {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }

    /// Pull out the entry data by move.
    pub fn into_entry_data(self) -> Option<(EntryHash, EntryType)> {
        match self {
            Self::Create(Create {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::Update(Update {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }

    pub fn entry_hash(&self) -> Option<&EntryHash> {
        self.entry_data().map(|d| d.0)
    }

    pub fn entry_type(&self) -> Option<&EntryType> {
        self.entry_data().map(|d| d.1)
    }

    pub fn action_type(&self) -> ActionType {
        self.into()
    }

    /// returns the public key of the agent who signed this action.
    pub fn author(&self) -> &AgentPubKey {
        match_action!(self => |i| { &i.author })
    }

    /// returns the timestamp of when the action was created
    pub fn timestamp(&self) -> Timestamp {
        match_action!(self => |i| { i.timestamp })
    }

    /// returns the sequence ordinal of this action
    pub fn action_seq(&self) -> u32 {
        match self {
            // Dna is always 0
            Self::Dna(Dna { .. }) => 0,
            Self::AgentValidationPkg(AgentValidationPkg { action_seq, .. })
            | Self::InitZomesComplete(InitZomesComplete { action_seq, .. })
            | Self::CreateLink(CreateLink { action_seq, .. })
            | Self::DeleteLink(DeleteLink { action_seq, .. })
            | Self::Delete(Delete { action_seq, .. })
            | Self::CloseChain(CloseChain { action_seq, .. })
            | Self::OpenChain(OpenChain { action_seq, .. })
            | Self::Create(Create { action_seq, .. })
            | Self::Update(Update { action_seq, .. }) => *action_seq,
        }
    }

    /// returns the previous action except for the DNA action which doesn't have a previous
    pub fn prev_action(&self) -> Option<&ActionHash> {
        Some(match self {
            Self::Dna(Dna { .. }) => return None,
            Self::AgentValidationPkg(AgentValidationPkg { prev_action, .. }) => prev_action,
            Self::InitZomesComplete(InitZomesComplete { prev_action, .. }) => prev_action,
            Self::CreateLink(CreateLink { prev_action, .. }) => prev_action,
            Self::DeleteLink(DeleteLink { prev_action, .. }) => prev_action,
            Self::Delete(Delete { prev_action, .. }) => prev_action,
            Self::CloseChain(CloseChain { prev_action, .. }) => prev_action,
            Self::OpenChain(OpenChain { prev_action, .. }) => prev_action,
            Self::Create(Create { prev_action, .. }) => prev_action,
            Self::Update(Update { prev_action, .. }) => prev_action,
        })
    }

    pub fn is_genesis(&self) -> bool {
        self.action_seq() < POST_GENESIS_SEQ_THRESHOLD
    }

    pub fn rate_data(&self) -> RateWeight {
        match self {
            Self::CreateLink(CreateLink { weight, .. }) => weight.clone(),
            Self::Delete(Delete { weight, .. }) => weight.clone(),
            Self::Create(Create { weight, .. }) => weight.clone().into(),
            Self::Update(Update { weight, .. }) => weight.clone().into(),

            // all others are weightless
            Self::Dna(Dna { .. })
            | Self::AgentValidationPkg(AgentValidationPkg { .. })
            | Self::InitZomesComplete(InitZomesComplete { .. })
            | Self::DeleteLink(DeleteLink { .. })
            | Self::CloseChain(CloseChain { .. })
            | Self::OpenChain(OpenChain { .. }) => RateWeight::default(),
        }
    }

    pub fn entry_rate_data(&self) -> Option<EntryRateWeight> {
        match self {
            Self::Create(Create { weight, .. }) => Some(weight.clone()),
            Self::Update(Update { weight, .. }) => Some(weight.clone()),

            // There is a weight, but it doesn't have the extra info that
            // Entry rate data has, so return None
            Self::CreateLink(CreateLink { .. }) => None,
            Self::Delete(Delete { .. }) => None,

            // all others are weightless, so return zero weight
            Self::Dna(Dna { .. })
            | Self::AgentValidationPkg(AgentValidationPkg { .. })
            | Self::InitZomesComplete(InitZomesComplete { .. })
            | Self::DeleteLink(DeleteLink { .. })
            | Self::CloseChain(CloseChain { .. })
            | Self::OpenChain(OpenChain { .. }) => Some(EntryRateWeight::default()),
        }
    }
}

impl_hashable_content!(Action, Action);

/// Allows the internal action types to produce
/// a [`ActionHash`] from a reference to themselves.
macro_rules! impl_hashable_content_for_ref {
    ($n: ident) => {
        impl HashableContent for $n {
            type HashType = holo_hash::hash_type::Action;

            fn hash_type(&self) -> Self::HashType {
                use holo_hash::PrimitiveHashType;
                holo_hash::hash_type::Action::new()
            }

            fn hashable_content(&self) -> holo_hash::HashableContentBytes {
                let h = ActionRef::$n(self);
                let sb = SerializedBytes::from(UnsafeBytes::from(
                    holochain_serialized_bytes::encode(&h)
                        .expect("Could not serialize HashableContent"),
                ));
                holo_hash::HashableContentBytes::Content(sb)
            }
        }
    };
}

impl_hashable_content_for_ref!(Dna);
impl_hashable_content_for_ref!(AgentValidationPkg);
impl_hashable_content_for_ref!(InitZomesComplete);
impl_hashable_content_for_ref!(CreateLink);
impl_hashable_content_for_ref!(DeleteLink);
impl_hashable_content_for_ref!(CloseChain);
impl_hashable_content_for_ref!(OpenChain);
impl_hashable_content_for_ref!(Create);
impl_hashable_content_for_ref!(Update);
impl_hashable_content_for_ref!(Delete);

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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct EntryDefIndex(pub u8);

/// The Dna Action is always the first action in a source chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    // No previous action, because DNA is always first chain entry
    pub hash: DnaHash,
}

/// Action for an agent validation package, used to determine whether an agent
/// is allowed to participate in this DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AgentValidationPkg {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub membrane_proof: Option<MembraneProof>,
}

/// A action which declares that all zome init functions have successfully
/// completed, and the chain is ready for commits. Contains no explicit data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct InitZomesComplete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,
}

/// Declares that a metadata Link should be made between two EntryHashes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// When migrating to a new version of a DNA, this action is committed to the
/// new chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct OpenChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub prev_dna_hash: DnaHash,
}

/// When migrating to a new version of a DNA, this action is committed to the
/// old chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CloseChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub new_dna_hash: DnaHash,
}

/// A action which "speaks" Entry content into being. The same content can be
/// referenced by multiple such actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Create<W = EntryRateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,

    pub weight: W,
}

/// A action which specifies that some new Entry content is intended to be an
/// update to some old Entry.
///
/// This action semantically updates an entry to a new entry.
/// It has the following effects:
/// - Create a new Entry
/// - This is the action of that new entry
/// - Create a metadata relationship between the original entry and this new action
///
/// The original action is required to prevent update loops:
/// If you update A to B and B back to A, and then you don't know which one came first,
/// or how to break the loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
/// Via the associated [`crate::Op`], this also has an effect on Entries: namely,
/// that a previously published Entry will become inaccessible if all of its
/// Actions are marked deleted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// Placeholder for future when we want to have updates on actions
/// Not currently in use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UpdateAction {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub original_action_address: ActionHash,
}

/// Placeholder for future when we want to have deletes on actions
/// Not currently in use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DeleteAction {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    /// Address of the action being deleted
    pub deletes_address: ActionHash,
}

/// Allows Actions which reference Entries to know what type of Entry it is
/// referencing. Useful for examining Actions without needing to fetch the
/// corresponding Entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum EntryType {
    /// An AgentPubKey
    AgentPubKey,
    /// An app-provided entry, along with its app-provided AppEntryType
    App(AppEntryType),
    /// A Capability claim
    CapClaim,
    /// A Capability grant.
    CapGrant,
}

impl EntryType {
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            EntryType::AgentPubKey => &EntryVisibility::Public,
            EntryType::App(t) => t.visibility(),
            EntryType::CapClaim => &EntryVisibility::Private,
            EntryType::CapGrant => &EntryVisibility::Private,
        }
    }
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryType::AgentPubKey => writeln!(f, "AgentPubKey"),
            EntryType::App(aet) => writeln!(f, "App({:?}, {:?})", aet.index(), aet.visibility()),
            EntryType::CapClaim => writeln!(f, "CapClaim"),
            EntryType::CapGrant => writeln!(f, "CapGrant"),
        }
    }
}

/// Information about a class of Entries provided by the DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppEntryType {
    /// A unique u8 identifier within a zome for this
    /// entry type.
    pub index: EntryDefIndex,
    /// The id of the zome that defines this entry type.
    pub zome_index: ZomeIndex,
    // @todo don't do this, use entry defs instead
    /// The visibility of this app entry.
    pub visibility: EntryVisibility,
}

impl AppEntryType {
    pub fn new(index: EntryDefIndex, zome_index: ZomeIndex, visibility: EntryVisibility) -> Self {
        Self {
            index,
            zome_index,
            visibility,
        }
    }

    pub fn index(&self) -> EntryDefIndex {
        self.index
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

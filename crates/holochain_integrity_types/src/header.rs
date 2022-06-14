use std::borrow::Borrow;

use crate::entry_def::EntryVisibility;
use crate::link::LinkTag;
use crate::link::LinkType;
use crate::timestamp::Timestamp;
use crate::EntryRateWeight;
use crate::GlobalZomeTypeId;
use crate::MembraneProof;
use crate::RateWeight;
use holo_hash::impl_hashable_content;
use holo_hash::AgentPubKey;
use holo_hash::AnyLinkableHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holo_hash::HashableContent;
use holo_hash::HeaderHash;
use holo_hash::HoloHashed;
use holochain_serialized_bytes::prelude::*;

pub mod builder;
pub mod conversions;

/// Any header with a header_seq less than this value is part of an element
/// created during genesis. Anything with this seq or higher was created
/// after genesis.
pub const POST_GENESIS_SEQ_THRESHOLD: u32 = 3;

/// Header contains variants for each type of header.
///
/// This struct really defines a local source chain, in the sense that it
/// implements the pointers between hashes that a hash chain relies on, which
/// are then used to check the integrity of data using cryptographic hash
/// functions.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[serde(tag = "type")]
pub enum Header {
    // The first header in a chain (for the DNA) doesn't have a previous header
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
/// This allows header types to be serialized to bytes without requiring
/// an owned value. This produces the same bytes as if they were
/// serialized with the [`Header`] type.
pub(crate) enum HeaderRef<'a> {
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

pub type HeaderHashed = HoloHashed<Header>;

/// a utility wrapper to write intos for our data types
macro_rules! write_into_header {
    ($($n:ident $(<$w : ty>)?),*,) => {

        /// A unit enum which just maps onto the different Header variants,
        /// without containing any extra data
        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Eq, Clone, Debug)]
        #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
        pub enum HeaderType {
            $($n,)*
        }

        impl std::fmt::Display for HeaderType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{}",
                    match self {
                        $( HeaderType::$n => stringify!($n), )*
                    }
                )
            }
        }

        impl From<&Header> for HeaderType {
            fn from(header: &Header) -> HeaderType {
                match header {
                    $(
                        Header::$n(_) => HeaderType::$n,
                    )*
                }
            }
        }
    };
}

/// A trait to unify the "inner" parts of a Header, i.e. the structs inside
/// the Header enum's variants. This trait is used for the "weighed" version
/// of each struct, i.e. the version without weight information erased.
///
/// Header types with no weight are considered "weighed" and "unweighed" at the
/// same time, but types with weight have distinct types for the weighed and
/// unweighed versions.
pub trait HeaderWeighed {
    type Unweighed: HeaderUnweighed;
    type Weight: Default;

    /// Construct the full Header enum with this variant.
    fn into_header(self) -> Header;

    /// Erase the rate limiting weight info, creating an "unweighed" version
    /// of this header. This is used primarily by validators who need to run the
    /// `weigh` callback on a header they received, and want to make sure their
    /// callback is not using the predefined weight to influence the result.
    fn unweighed(self) -> Self::Unweighed;
}

/// A trait to unify the "inner" parts of a Header, i.e. the structs inside
/// the Header enum's variants. This trait is used for the "unweighed" version
/// of each struct, i.e. the version with weight information erased.
///
/// Header types with no weight are considered "weighed" and "unweighed" at the
/// same time, but types with weight have distinct types for the weighed and
/// unweighed versions.
pub trait HeaderUnweighed: Sized {
    type Weighed: HeaderWeighed;
    type Weight: Default;

    /// Add a weight to this unweighed header, making it "weighed".
    /// The weight is determined by the `weigh` callback, which is run on the
    /// unweighed version of this header.
    fn weighed(self, weight: Self::Weight) -> Self::Weighed;

    /// Add zero weight to this unweighed header, making it "weighed".
    #[cfg(feature = "test_utils")]
    fn weightless(self) -> Self::Weighed {
        self.weighed(Default::default())
    }
}

impl<I: HeaderWeighed> From<I> for Header {
    fn from(i: I) -> Self {
        i.into_header()
    }
}

write_into_header! {
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
macro_rules! match_header {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            Header::Dna($i) => { $($t)* }
            Header::AgentValidationPkg($i) => { $($t)* }
            Header::InitZomesComplete($i) => { $($t)* }
            Header::CreateLink($i) => { $($t)* }
            Header::DeleteLink($i) => { $($t)* }
            Header::OpenChain($i) => { $($t)* }
            Header::CloseChain($i) => { $($t)* }
            Header::Create($i) => { $($t)* }
            Header::Update($i) => { $($t)* }
            Header::Delete($i) => { $($t)* }
        }
    };
}

impl Header {
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

    pub fn header_type(&self) -> HeaderType {
        self.into()
    }

    /// returns the public key of the agent who signed this header.
    pub fn author(&self) -> &AgentPubKey {
        match_header!(self => |i| { &i.author })
    }

    /// returns the timestamp of when the header was created
    pub fn timestamp(&self) -> Timestamp {
        match_header!(self => |i| { i.timestamp })
    }

    /// returns the sequence ordinal of this header
    pub fn header_seq(&self) -> u32 {
        match self {
            // Dna is always 0
            Self::Dna(Dna { .. }) => 0,
            Self::AgentValidationPkg(AgentValidationPkg { header_seq, .. })
            | Self::InitZomesComplete(InitZomesComplete { header_seq, .. })
            | Self::CreateLink(CreateLink { header_seq, .. })
            | Self::DeleteLink(DeleteLink { header_seq, .. })
            | Self::Delete(Delete { header_seq, .. })
            | Self::CloseChain(CloseChain { header_seq, .. })
            | Self::OpenChain(OpenChain { header_seq, .. })
            | Self::Create(Create { header_seq, .. })
            | Self::Update(Update { header_seq, .. }) => *header_seq,
        }
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header(&self) -> Option<&HeaderHash> {
        Some(match self {
            Self::Dna(Dna { .. }) => return None,
            Self::AgentValidationPkg(AgentValidationPkg { prev_header, .. }) => prev_header,
            Self::InitZomesComplete(InitZomesComplete { prev_header, .. }) => prev_header,
            Self::CreateLink(CreateLink { prev_header, .. }) => prev_header,
            Self::DeleteLink(DeleteLink { prev_header, .. }) => prev_header,
            Self::Delete(Delete { prev_header, .. }) => prev_header,
            Self::CloseChain(CloseChain { prev_header, .. }) => prev_header,
            Self::OpenChain(OpenChain { prev_header, .. }) => prev_header,
            Self::Create(Create { prev_header, .. }) => prev_header,
            Self::Update(Update { prev_header, .. }) => prev_header,
        })
    }

    pub fn is_genesis(&self) -> bool {
        self.header_seq() < POST_GENESIS_SEQ_THRESHOLD
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

impl_hashable_content!(Header, Header);

/// Allows the internal header types to produce
/// a [`HeaderHash`] from a reference to themselves.
macro_rules! impl_hashable_content_for_ref {
    ($n: ident) => {
        impl HashableContent for $n {
            type HashType = holo_hash::hash_type::Header;

            fn hash_type(&self) -> Self::HashType {
                use holo_hash::PrimitiveHashType;
                holo_hash::hash_type::Header::new()
            }

            fn hashable_content(&self) -> holo_hash::HashableContentBytes {
                let h = HeaderRef::$n(self);
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
pub struct ZomeId(pub u8);

impl ZomeId {
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

/// The Dna Header is always the first header in a source chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    // No previous header, because DNA is always first chain entry
    pub hash: DnaHash,
}

/// Header for an agent validation package, used to determine whether an agent
/// is allowed to participate in this DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AgentValidationPkg {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub membrane_proof: Option<MembraneProof>,
}

/// A header which declares that all zome init functions have successfully
/// completed, and the chain is ready for commits. Contains no explicit data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct InitZomesComplete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
}

/// Declares that a metadata Link should be made between two EntryHashes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CreateLink<W = RateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub base_address: AnyLinkableHash,
    pub target_address: AnyLinkableHash,
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
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    /// this is redundant with the `CreateLink` header but needs to be included to facilitate DHT ops
    /// this is NOT exposed to wasm developers and is validated by the subconscious to ensure that
    /// it always matches the `base_address` of the `CreateLink`
    pub base_address: AnyLinkableHash,
    /// The address of the `CreateLink` being reversed
    pub link_add_address: HeaderHash,
}

/// When migrating to a new version of a DNA, this header is committed to the
/// new chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct OpenChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub prev_dna_hash: DnaHash,
}

/// When migrating to a new version of a DNA, this header is committed to the
/// old chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CloseChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub new_dna_hash: DnaHash,
}

/// A header which "speaks" Entry content into being. The same content can be
/// referenced by multiple such headers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Create<W = EntryRateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,

    pub weight: W,
}

/// A header which specifies that some new Entry content is intended to be an
/// update to some old Entry.
///
/// This header semantically updates an entry to a new entry.
/// It has the following effects:
/// - Create a new Entry
/// - This is the header of that new entry
/// - Create a metadata relationship between the original entry and this new header
///
/// The original header is required to prevent update loops:
/// If you update A to B and B back to A, and then you don't know which one came first,
/// or how to break the loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Update<W = EntryRateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub original_header_address: HeaderHash,
    pub original_entry_address: EntryHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,

    pub weight: W,
}

/// Declare that a previously published Header should be nullified and
/// considered deleted.
///
/// Via the associated [`crate::Op`], this also has an effect on Entries: namely,
/// that a previously published Entry will become inaccessible if all of its
/// Headers are marked deleted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Delete<W = RateWeight> {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    /// Address of the Element being deleted
    pub deletes_address: HeaderHash,
    pub deletes_entry_address: EntryHash,

    pub weight: W,
}

/// Placeholder for future when we want to have updates on headers
/// Not currently in use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UpdateHeader {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub original_header_address: HeaderHash,
}

/// Placeholder for future when we want to have deletes on headers
/// Not currently in use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DeleteHeader {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    /// Address of the header being deleted
    pub deletes_address: HeaderHash,
}

/// Allows Headers which reference Entries to know what type of Entry it is
/// referencing. Useful for examining Headers without needing to fetch the
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
            EntryType::App(aet) => writeln!(f, "App({:?}, {:?})", aet.id(), aet.visibility()),
            EntryType::CapClaim => writeln!(f, "CapClaim"),
            EntryType::CapGrant => writeln!(f, "CapGrant"),
        }
    }
}

/// Information about a class of Entries provided by the DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppEntryType {
    /// u8 identifier of what entry type this is
    /// this is a unique global identifier across the
    /// DNA for this type. It is a [`GlobalZomeTypeId`].
    pub id: EntryDefIndex,
    // @todo don't do this, use entry defs instead
    /// The visibility of this app entry.
    pub visibility: EntryVisibility,
}

impl AppEntryType {
    pub fn new(id: EntryDefIndex, visibility: EntryVisibility) -> Self {
        Self { id, visibility }
    }

    pub fn id(&self) -> EntryDefIndex {
        self.id
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

impl ZomeId {
    /// Use as an index into a slice
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

impl std::ops::Deref for ZomeId {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<u8> for ZomeId {
    fn borrow(&self) -> &u8 {
        &self.0
    }
}

impl From<EntryDefIndex> for GlobalZomeTypeId {
    fn from(v: EntryDefIndex) -> Self {
        Self(v.0)
    }
}

impl From<GlobalZomeTypeId> for EntryDefIndex {
    fn from(v: GlobalZomeTypeId) -> Self {
        Self(v.0)
    }
}

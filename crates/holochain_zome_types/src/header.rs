use crate::entry_def::EntryVisibility;
use crate::link::LinkTag;
use crate::timestamp::Timestamp;
pub use builder::HeaderBuilder;
pub use builder::HeaderBuilderCommon;
use holo_hash::impl_hashable_content;
use holo_hash::AgentPubKey;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct HeaderHashes(pub Vec<HeaderHash>);

impl From<Vec<HeaderHash>> for HeaderHashes {
    fn from(vs: Vec<HeaderHash>) -> Self {
        Self(vs)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct HeaderHashedVec(pub Vec<HeaderHashed>);

impl From<Vec<HeaderHashed>> for HeaderHashedVec {
    fn from(vs: Vec<HeaderHashed>) -> Self {
        Self(vs)
    }
}

/// Header contains variants for each type of header.
///
/// This struct really defines a local source chain, in the sense that it
/// implements the pointers between hashes that a hash chain relies on, which
/// are then used to check the integrity of data using cryptographic hash
/// functions.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
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

pub type HeaderHashed = HoloHashed<Header>;

/// a utility wrapper to write intos for our data types
macro_rules! write_into_header {
    ($($n:ident),*,) => {
        $(
            impl HeaderInner for $n {
                fn into_header(self) -> Header {
                    Header::$n(self)
                }
            }
        )*

        /// A unit enum which just maps onto the different Header variants,
        /// without containing any extra data
        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Clone, Debug)]
        pub enum HeaderType {
            $($n,)*
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

/// A trait to specify the common parts of a Header
pub trait HeaderInner {
    /// Get a full header from the subset
    fn into_header(self) -> Header;
}

impl<I: HeaderInner> From<I> for Header {
    fn from(i: I) -> Self {
        i.into_header()
    }
}

write_into_header! {
    Dna,
    AgentValidationPkg,
    InitZomesComplete,
    CreateLink,
    DeleteLink,
    OpenChain,
    CloseChain,
    Create,
    Update,
    Delete,
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
}

impl_hashable_content!(Header, Header);

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
pub struct ZomeId(u8);

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

/// The Dna Header is always the first header in a source chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    // No previous header, because DNA is always first chain entry
    pub hash: DnaHash,
}

/// Header for an agent validation package, used to determine whether an agent
/// is allowed to participate in this DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AgentValidationPkg {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub membrane_proof: Option<SerializedBytes>,
}

/// A header which declares that all zome init functions have successfully
/// completed, and the chain is ready for commits. Contains no explicit data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct InitZomesComplete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,
}

/// Declares that a metadata Link should be made between two EntryHashes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct CreateLink {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub base_address: EntryHash,
    pub target_address: EntryHash,
    pub zome_id: ZomeId,
    pub tag: LinkTag,
}

/// Declares that a previously made Link should be nullified and considered removed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct DeleteLink {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    /// this is redundant with the `CreateLink` header but needs to be included to facilitate DHT ops
    /// this is NOT exposed to wasm developers and is validated by the subconscious to ensure that
    /// it always matches the `base_address` of the `CreateLink`
    pub base_address: EntryHash,
    /// The address of the `CreateLink` being reversed
    pub link_add_address: HeaderHash,
}

/// When migrating to a new version of a DNA, this header is committed to the
/// new chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct OpenChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub prev_dna_hash: DnaHash,
}

/// When migrating to a new version of a DNA, this header is committed to the
/// old chain to declare the migration path taken. **Currently unused**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
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
pub struct Create {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
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
pub struct Update {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub original_header_address: HeaderHash,
    pub original_entry_address: EntryHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

/// Declare that a previously published Header should be nullified and
/// considered deleted.
///
/// Via the associated [DhtOp], this also has an effect on Entries: namely,
/// that a previously published Entry will become inaccessible if all of its
/// Headers are marked deleted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct Delete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    /// Address of the Element being deleted
    pub deletes_address: HeaderHash,
    pub deletes_entry_address: EntryHash,
}

/// Placeholder for future when we want to have updates on headers
/// Not currently in use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
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
            EntryType::App(t) => &t.visibility(),
            EntryType::CapClaim => &EntryVisibility::Private,
            EntryType::CapGrant => &EntryVisibility::Private,
        }
    }
}

/// Information about a class of Entries provided by the DNA
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct AppEntryType {
    /// u8 identifier of what entry type this is
    /// this needs to match the position of the entry type returned by entry defs
    pub(crate) id: EntryDefIndex,
    /// u8 identifier of what zome this is for
    /// this needs to be shared across the dna
    /// comes from the numeric index position of a zome in dna config
    pub(crate) zome_id: ZomeId,
    // @todo don't do this, use entry defs instead
    pub(crate) visibility: EntryVisibility,
}

impl AppEntryType {
    pub fn new(id: EntryDefIndex, zome_id: ZomeId, visibility: EntryVisibility) -> Self {
        Self {
            id,
            zome_id,
            visibility,
        }
    }

    pub fn id(&self) -> EntryDefIndex {
        self.id
    }
    pub fn zome_id(&self) -> ZomeId {
        self.zome_id
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

impl EntryDefIndex {
    /// Use as an index into a slice
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

impl ZomeId {
    /// Use as an index into a slice
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

/// Creating a Header requires certain details, which may be deduced at creation time, or may be
/// supplied by the caller.  For example, to reconstruct a source-chain from backup, or to commit a
/// header with a specific Timestamp or at a known location or sequence in the source-chain to
/// implement "atomic" read-modify-write algorithms.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash)]
pub struct HeaderDetails {
    pub timestamp: Option<Timestamp>,
    pub header_seq: Option<u32>,
    pub prev_header: Option<HeaderHash>,
}

//! Holochain's [`Header`] and its variations.
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::composite_hash::{EntryHash, HeaderAddress};
use crate::{link::Tag, prelude::*};
use holochain_zome_types::entry_def::EntryVisibility;

pub mod builder;
pub use builder::{HeaderBuilder, HeaderBuilderCommon};

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
    LinkAdd(LinkAdd),
    LinkRemove(LinkRemove),
    ChainOpen(ChainOpen),
    ChainClose(ChainClose),
    EntryCreate(EntryCreate),
    EntryUpdate(EntryUpdate),
    EntryDelete(EntryDelete),
}

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
    };
}

pub trait HeaderInner {
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
    LinkAdd,
    LinkRemove,
    ChainOpen,
    ChainClose,
    EntryCreate,
    EntryUpdate,
    EntryDelete,
}

/// a utility macro just to not have to type in the match statement everywhere.
macro_rules! match_header {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            Header::Dna($i) => { $($t)* }
            Header::AgentValidationPkg($i) => { $($t)* }
            Header::InitZomesComplete($i) => { $($t)* }
            Header::LinkAdd($i) => { $($t)* }
            Header::LinkRemove($i) => { $($t)* }
            Header::ChainOpen($i) => { $($t)* }
            Header::ChainClose($i) => { $($t)* }
            Header::EntryCreate($i) => { $($t)* }
            Header::EntryUpdate($i) => { $($t)* }
            Header::EntryDelete($i) => { $($t)* }
        }
    };
}

impl Header {
    /// Returns the address and entry type of the Entry, if applicable.
    // TODO: DRY: possibly create an `EntryData` struct which is used by both
    // EntryCreate and EntryUpdate
    pub fn entry_data(&self) -> Option<(&EntryHash, &EntryType)> {
        match self {
            Self::EntryCreate(EntryCreate {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::EntryUpdate(EntryUpdate {
                entry_hash,
                entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
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
        match_header!(self => |i| { i.header_seq })
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header(&self) -> Option<&HeaderAddress> {
        Some(match self {
            Self::Dna(Dna { .. }) => return None,
            Self::AgentValidationPkg(AgentValidationPkg { prev_header, .. }) => prev_header,
            Self::InitZomesComplete(InitZomesComplete { prev_header, .. }) => prev_header,
            Self::LinkAdd(LinkAdd { prev_header, .. }) => prev_header,
            Self::LinkRemove(LinkRemove { prev_header, .. }) => prev_header,
            Self::EntryDelete(EntryDelete { prev_header, .. }) => prev_header,
            Self::ChainClose(ChainClose { prev_header, .. }) => prev_header,
            Self::ChainOpen(ChainOpen { prev_header, .. }) => prev_header,
            Self::EntryCreate(EntryCreate { prev_header, .. }) => prev_header,
            Self::EntryUpdate(EntryUpdate { prev_header, .. }) => prev_header,
        })
    }
}

make_hashed_base! {
    Visibility(pub),
    HashedName(HeaderHashed),
    ContentType(Header),
    HashType(HeaderHash),
}

impl HeaderHashed {
    pub async fn with_data(header: Header) -> Result<Self, SerializedBytesError> {
        let sb = SerializedBytes::try_from(&header)?;
        Ok(HeaderHashed::with_pre_hashed(
            header,
            HeaderHash::with_data(UnsafeBytes::from(sb).into()).await,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
/// A header of one of the two types that create a new entry.
pub enum NewEntryHeader {
    Create(EntryCreate),
    Update(EntryUpdate),
}

impl NewEntryHeader {
    /// Get the entry on this header
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryHeader::Create(EntryCreate { entry_hash, .. })
            | NewEntryHeader::Update(EntryUpdate { entry_hash, .. }) => entry_hash,
        }
    }
}

impl From<NewEntryHeader> for Header {
    fn from(h: NewEntryHeader) -> Self {
        match h {
            NewEntryHeader::Create(h) => Header::EntryCreate(h),
            NewEntryHeader::Update(h) => Header::EntryUpdate(h),
        }
    }
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
    Serialize,
    Deserialize,
    SerializedBytes,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
)]
pub struct ZomeId(u8);

/// Specifies whether an [EntryUpdate] refers to an [Entry] or a [Header]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum UpdateBasis {
    Header,
    Entry,
}

/// header for a DNA entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    // No previous header, because DNA is always first chain entry
    pub hash: DnaHash,
}

/// header for a agent validation entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AgentValidationPkg {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub membrane_proof: Option<SerializedBytes>,
}

/// header for a zome init entry to mark chain ready.  Contains no entry data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct InitZomesComplete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkAdd {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub base_address: EntryHash,
    pub target_address: EntryHash,
    pub zome_id: ZomePosition,
    pub tag: Tag,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkRemove {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
    pub base_address: EntryHash,
    /// The address of the `LinkAdd` being reversed
    pub link_add_address: HeaderAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainOpen {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub prev_dna_hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainClose {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub new_dna_hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryCreate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryUpdate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub update_basis: UpdateBasis,
    pub replaces_address: HeaderHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryDelete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    /// Address of the Element being deleted
    pub removes_address: HeaderAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum EntryType {
    AgentPubKey,
    // Stores the App's provided filtration data
    // FIXME: Change this if we are keeping Zomes
    App(AppEntryType),
    CapClaim,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AppEntryType {
    /// u8 identifier of what entry type this is
    /// this needs to match the position of the entry type returned by entry defs
    pub(crate) id: EntryDefPosition,
    /// u8 identifier of what zome this is for
    /// this needs to be shared across the dna
    /// comes from the numeric index position of a zome in dna config
    pub(crate) zome_id: ZomePosition,
    // @todo don't do this, use entry defs instead
    pub(crate) visibility: EntryVisibility,
}

impl AppEntryType {
    pub fn new(id: EntryDefPosition, zome_id: ZomePosition, visibility: EntryVisibility) -> Self {
        Self {
            id,
            zome_id,
            visibility,
        }
    }

    pub fn id(&self) -> EntryDefPosition {
        self.id
    }
    pub fn zome_id(&self) -> ZomePosition {
        self.zome_id
    }
    pub fn visibility(&self) -> &EntryVisibility {
        &self.visibility
    }
}

impl Dna {
    /// Dna cannot implement the trait as it doesn't have a previous header
    pub fn from_builder(hash: DnaHash, builder: HeaderBuilderCommon) -> Self {
        Self {
            author: builder.author,
            timestamp: builder.timestamp,
            header_seq: builder.header_seq,
            hash,
        }
    }
}

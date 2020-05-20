//! Holochain's [`Header`] and its variations.
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::address::{DhtAddress, EntryAddress, HeaderAddress};

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
            impl From<$n> for Header {
                fn from(n: $n) -> Self {
                    Self::$n(n)
                }
            }
        )*
    };
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
    /// Returns `false` if this header is associated with a private entry. Otherwise, returns `true`.
    pub fn entry_type(&self) -> Option<&EntryType> {
        match self {
            Self::EntryCreate(EntryCreate { entry_type, .. }) => Some(entry_type),
            Self::EntryUpdate(EntryUpdate { entry_type, .. }) => Some(entry_type),
            _ => None,
        }
    }

    /// Returns the public key of the agent who signed this header.
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
    HashType(HeaderAddress),
}

impl HeaderHashed {
    pub async fn with_data(header: Header) -> Result<Self, SerializedBytesError> {
        let sb = SerializedBytes::try_from(&header)?;
        Ok(HeaderHashed::with_pre_hashed(
            header,
            HeaderAddress::Header(HeaderHash::with_data(sb.bytes()).await),
        ))
    }
}

/// this id in an internal reference, which also serves as a canonical ordering
/// for zome initialization.  The value should be auto-generated from the Zome Bundle def
pub type ZomeId = u8;

use crate::prelude::*;

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

    pub base_address: DhtAddress,
    pub target_address: DhtAddress,
    pub tag: SerializedBytes,
    pub link_type: SerializedBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkRemove {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,
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
    pub entry_address: EntryAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryUpdate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    pub replaces_address: DhtAddress,

    pub entry_type: EntryType,
    pub entry_address: EntryAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryDelete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderAddress,

    /// Address of the Element being deleted
    pub removes_address: DhtAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum EntryType {
    AgentPubKey,
    // Stores the App's provided filtration data
    // FIXME: Change this if we are keeping Zomes
    App(AppEntryType),
    CapTokenClaim,
    CapTokenGrant,
}

impl EntryType {
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            EntryType::AgentPubKey => &EntryVisibility::Public,
            EntryType::App(t) => &t.visibility,
            EntryType::CapTokenClaim => &EntryVisibility::Private,
            EntryType::CapTokenGrant => &EntryVisibility::Private,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AppEntryType {
    pub(crate) id: Vec<u8>,
    pub(crate) zome_id: ZomeId,
    pub(crate) visibility: EntryVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum EntryVisibility {
    Public,
    Private,
}

impl EntryVisibility {
    pub fn is_public(&self) -> bool {
        *self == EntryVisibility::Public
    }
}

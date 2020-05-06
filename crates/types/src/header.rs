//! Holochain's [`Header`] and its variations.
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::address::{DhtAddress, EntryAddress, HeaderAddress};

#[derive(Clone, Debug, shrinkwraprs::Shrinkwrap, PartialEq)]
pub struct Header {
    #[shrinkwrap(main_field)]
    header_type: HeaderType,
    header_hash: HeaderHash,
}

impl Header {
    pub async fn new(header_type: HeaderType) -> Result<Self, SerializedBytesError> {
        let sb: SerializedBytes = header_type.clone().try_into()?;
        let header_hash = HeaderHash::with_data(&sb.bytes()).await;
        Ok(Self {
            header_type,
            header_hash,
        })
    }

    pub fn hash(&self) -> &HeaderHash {
        &self.header_hash
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
pub enum HeaderType {
    // The first header in a chain (for the DNA) doesn't have a previous header
    Dna(Dna),
    LinkAdd(LinkAdd),
    LinkRemove(LinkRemove),
    ChainOpen(ChainOpen),
    ChainClose(ChainClose),
    EntryCreate(EntryCreate),
    EntryUpdate(EntryUpdate),
    EntryDelete(EntryDelete),
}

macro_rules! from_data_struct {
    ($($n:ident),*,) => {
        $(
            impl From<$n> for HeaderType {
                fn from(i: $n) -> Self {
                    Self::$n(i)
                }
            }
        )*
    };
}

from_data_struct! {
    Dna,
    LinkAdd,
    LinkRemove,
    ChainOpen,
    ChainClose,
    EntryCreate,
    EntryUpdate,
    EntryDelete,
}

impl HeaderType {
    /// Returns `false` if this header is associated with a private entry. Otherwise, returns `true`.
    pub fn is_public(&self) -> bool {
        unimplemented!()
    }

    /// Returns the public key of the agent who signed this header.
    pub fn author(&self) -> &AgentPubKey {
        match self {
            Self::Dna(i) => &i.author,
            Self::LinkAdd(i) => &i.author,
            Self::LinkRemove(i) => &i.author,
            Self::ChainOpen(i) => &i.author,
            Self::ChainClose(i) => &i.author,
            Self::EntryCreate(i) => &i.author,
            Self::EntryUpdate(i) => &i.author,
            Self::EntryDelete(i) => &i.author,
        }
    }

    /// returns the timestamp of when the header was created
    pub fn timestamp(&self) -> Timestamp {
        unimplemented!()
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header(&self) -> Option<&HeaderAddress> {
        Some(match self {
            Self::Dna(Dna { .. }) => return None,
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

/// this id in an internal reference, which also serves as a canonical ordering
/// for zome initialization.  The value should be auto-generated from the Zome Bundle def
pub type ZomeId = u8;

use crate::prelude::*;

/// header for a DNA entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct Dna {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    // No previous header, because DNA is always first chain entry
    pub hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkAdd {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
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
    pub prev_header: HeaderAddress,
    /// The address of the `LinkAdd` being reversed
    pub link_add_address: HeaderAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainOpen {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderAddress,

    pub prev_dna_hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainClose {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderAddress,

    pub new_dna_hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryCreate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderAddress,

    pub entry_type: EntryType,
    pub entry_address: EntryAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryUpdate {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderAddress,

    pub replaces_address: DhtAddress,

    pub entry_type: EntryType,
    pub entry_address: EntryAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryDelete {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AppEntryType {
    id: Vec<u8>,
    zome_id: ZomeId,
    is_public: bool,
}

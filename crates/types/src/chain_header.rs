//! This module contains definitions of the ChainHeader struct, constructor, and getters. This struct really defines a local source chain,
//! in the sense that it implements the pointers between hashes that a hash chain relies on, which
//! are then used to check the integrity of data using cryptographic hash functions.

use crate::{
    entry::{Entry, EntryAddress},
    signature::Signature,
    header,
    prelude::*,
};
use shrinkwraprs::Shrinkwrap;

/// wraps header hash to promote it to an "address" e.g. for use in a CAS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HeaderAddress {
    /// a header hash, the only option
    Header(HeaderHash),
}

impl From<HeaderAddress> for HoloHash {
    fn from(header_address: HeaderAddress) -> HoloHash {
        match header_address {
            HeaderAddress::Header(header_hash) => header_hash.into(),
        }
    }
}

impl From<holo_hash::holo_hash_core::HeaderHash> for HeaderAddress {
    fn from(header_hash: holo_hash::holo_hash_core::HeaderHash) -> HeaderAddress {
        holo_hash::HeaderHash::from(header_hash).into()
    }
}

impl From<HeaderHash> for HeaderAddress {
    fn from(header_hash: HeaderHash) -> HeaderAddress {
        HeaderAddress::Header(header_hash)
    }
}

impl std::convert::TryFrom<&ChainHeader> for HeaderAddress {
    type Error = SerializedBytesError;
    fn try_from(chain_header: &ChainHeader) -> Result<Self, Self::Error> {
        Ok(HeaderAddress::Header(HeaderHash::try_from(chain_header)?))
    }
}

/// ChainHeader + Entry.
pub struct HeaderWithEntry(ChainHeader, Entry);

impl HeaderWithEntry {
    /// HeaderWithEntry constructor.
    pub fn new(header: ChainHeader, entry: Entry) -> Self {
        Self(header, entry)
    }

    /// Access the ChainHeader portion of this pair.
    pub fn header(&self) -> &ChainHeader {
        &self.0
    }

    /// Access the Entry portion of this pair.
    pub fn entry(&self) -> &Entry {
        &self.1
    }
}

/// a chain element which is a triple containing the signature of the header along with the
/// entry if the header type has one.
pub struct ChainElement(Signature, ChainHeader, Option<Entry>);

/// the header and the signature that signed it
#[derive(Shrinkwrap)]
pub struct SignedHeader {
    #[shrinkwrap(main_field)] header: ChainHeader,
    signature: Signature,
}

impl ChainElement {
    /// Element constructor.
    pub fn new(signature: Signature, header: ChainHeader, maybe_entry: Option<Entry>) -> Self {
        Self(signature, header, entry)
    }

    /// Access the signature portion of this triple.
    pub fn header(&self) -> &Signature {
        &self.0
    }

    /// Access the ChainHeader portion of this triple.
    pub fn header(&self) -> &ChainHeader {
        &self.1
    }

    /// Access the Entry portion of this triple.
    pub fn entry(&self) -> Option<&Entry> {
        &self.2
    }
}

/// ChainHeader contains variants for each type of header.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum ChainHeader {
    // The first header in a chain (for the DNA) doesn't have a previous header
    Dna(header::Dna),
    LinkAdd(header::LinkAdd),
    LinkRemove(header::LinkRemove),
    ChainOpen(header::ChainOpen),
    ChainClose(header::ChainClose),
    EntryCreate(header::EntryCreate),
    EntryUpdate(header::EntryUpdate),
    EntryDelete(header::EntryDelete),
}

impl ChainHeader {

    pub fn new(author: AgentHash,  prev_header: Option<HeaderHash>, entry: Option<Entry>,
    ) -> Result<Self,> {
        let timestamp: Iso8601 = chrono::Utc::now().timestamp().into();
        match entry {
            Entry::Dna(dna) => header::Dna{author, timestamp, hash: dna.hash() },
//            Entry::AgentKey(agent_hash) => header::
            App(entry) => header::
        }
    }

    fn header_for_entry(
        entry: &Entry,
        agent_hash: &AgentHash,
        prev_head: Option<HeaderAddress>,
    ) -> Result<ChainHeader, SerializedBytesError> {
        let _provenances = &[Provenance::new(agent_hash.clone(), Signature::fake())];
        let _timestamp: Iso8601 = chrono::Utc::now().timestamp().into();w
        Ok(ChainHeader {
            entry_address: EntryAddress::try_from(entry)?,
            prev_header_address: prev_head,
        })
    }


    /// Return the previous ChainHeader's Address in the chain
    pub fn prev_header_address(&self) -> Option<HeaderAddress> {
        self.prev_header().map(|h| h.to_owned().into())
    }

    /// Returns whether this header's entry data is public
    pub fn is_public(&self) -> bool {
        unimplemented!()
    }
    //    pub fn author() -> PublicKey { unimplemented!() }
    //   pub fn timestamp() -> Timestamp { unimplemented!() }

    /// Return the EntryAddress this header points to if it does (system actions don't have entries)
    pub fn entry_address(&self) -> Option<EntryAddress> {
        Some(match self {
            Self::Dna(header::Dna { hash, .. }) => EntryAddress::Dna(hash.to_owned()),
            Self::LinkAdd(header::LinkAdd { .. }) => return None,
            Self::LinkRemove(header::LinkRemove { .. }) => return None,
            Self::EntryDelete(header::EntryDelete { .. }) => return None,
            Self::ChainClose(header::ChainClose { .. }) => return None,
            Self::ChainOpen(header::ChainOpen { .. }) => return None,
            Self::EntryCreate(header::EntryCreate { entry_hash, .. }) => {
                EntryAddress::Entry(entry_hash.to_owned())
            }
            Self::EntryUpdate(header::EntryUpdate { entry_hash, .. }) => {
                EntryAddress::Entry(entry_hash.to_owned())
            }
        })
    }

    // FIXME: use async with_data, or consider wrapper type
    // https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r413226841
    /// calculates the hash of the header
    pub fn hash(&self) -> HeaderHash {
        // hash the header enum variant struct
        let sb: SerializedBytes = self.try_into().expect("TODO: can this fail?");
        HeaderHash::with_data_sync(&sb.bytes())
    }

    /// returns the previous header except for the DNA header which doesn't have a previous
    pub fn prev_header(&self) -> Option<&HeaderHash> {
        Some(match self {
            Self::Dna(header::Dna { .. }) => return None,
            Self::LinkAdd(header::LinkAdd { prev_header, .. }) => prev_header,
            Self::LinkRemove(header::LinkRemove { prev_header, .. }) => prev_header,
            Self::EntryDelete(header::EntryDelete { prev_header, .. }) => prev_header,
            Self::ChainClose(header::ChainClose { prev_header, .. }) => prev_header,
            Self::ChainOpen(header::ChainOpen { prev_header, .. }) => prev_header,
            Self::EntryCreate(header::EntryCreate { prev_header, .. }) => prev_header,
            Self::EntryUpdate(header::EntryUpdate { prev_header, .. }) => prev_header,
        })
    }
}

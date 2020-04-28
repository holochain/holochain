//! This module contains definitions of the ChainHeader struct, constructor, and getters. This struct really defines a local source chain,
//! in the sense that it implements the pointers between hashes that a hash chain relies on, which
//! are then used to check the integrity of data using cryptographic hash functions.

use crate::{entry::Entry, header, prelude::*};
use holochain_keystore::Signature;
use shrinkwraprs::Shrinkwrap;
use thiserror::Error;

/// Error type regarding chain elements
#[derive(Error, Debug, PartialEq)]
pub enum ChainElementError {
    /// Element signature doesn't validate against the header
    #[error("Element signature is invalid")]
    InvalidSignature,
    /// Element's header and entry types don't match
    #[error("Element header and entry type mismatch")]
    TypeMismatch,
}

/// Convenience result type for ChainElementError
pub type ChainElementResult<T> = Result<T, ChainElementError>;

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

/// a chain element which is a triple containing the signature of the header along with the
/// entry if the header type has one.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainElement(Signature, ChainHeader, Option<Entry>);

/// the header and the signature that signed it
#[derive(Shrinkwrap, Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code, missing_docs)]
pub struct SignedHeader {
    #[shrinkwrap(main_field)]
    pub header: ChainHeader,
    pub signature: Signature,
}

impl ChainElement {
    /// Element constructor.
    pub fn new(signature: Signature, header: ChainHeader, maybe_entry: Option<Entry>) -> Self {
        Self(signature, header, maybe_entry)
    }

    /// Validates a chain element
    pub fn validate(&self) -> ChainElementResult<()> {
        //TODO: gheck that signature is of the header:
        //      ChainElementError::InvalidSignature
        //TODO: make sure that any cases around entry existence are valid:
        //      ChainElementError::TypeMismatch,
        unimplemented!()
    }

    /// Access the signature portion of this triple.
    pub fn signature(&self) -> &Signature {
        &self.0
    }

    /// Access the ChainHeader portion of this triple.
    pub fn header(&self) -> &ChainHeader {
        &self.1
    }

    /// Access the Entry portion of this triple.
    pub fn entry(&self) -> &Option<Entry> {
        &self.2
    }
}

/// ChainHeader contains variants for each type of header.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[serde(tag = "type")]
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
    /// return the previous ChainHeader's Address in the chain
    pub fn prev_header_address(&self) -> Option<HeaderAddress> {
        self.prev_header().map(|h| h.to_owned().into())
    }

    /// returns whether this header's entry data is public
    pub fn is_public(&self) -> bool {
        unimplemented!()
    }

    /// returns the author who signed the header
    pub fn author() -> AgentHash {
        unimplemented!()
    }

    /// returns the timestamp of when the header was created
    pub fn timestamp() -> header::Timestamp {
        unimplemented!()
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

//! This module contains definitions of the ChainHeader struct, constructor, and getters. This struct really defines a local source chain,
//! in the sense that it implements the pointers between hashes that a hash chain relies on, which
//! are then used to check the integrity of data using cryptographic hash functions.

use crate::{
    entry::{Entry, EntryAddress},
    prelude::*,
};

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

/// The portion of the chain header that is serialized + signed
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[serde(tag = "header_type", content = "content")]
pub enum ChainHeaderContent {
    /// Fill these types in with the actual holochain formalization
    /// header types.
    StubHeaderType {
        /// delete the option here - the previous header this header chains from
        prev_header_address: Option<HeaderAddress>,

        /// the entry address this header points to
        entry_address: EntryAddress,
    }
}

/// This type encompases a header's content and any signatures.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainHeader {
    /// The content of the header
    pub content: ChainHeaderContent,

    /// The signatures
    pub provenance: Vec<crate::signature::Provenance>,
}

impl ChainHeader {
    /// Return the EntryHash this header points to
    pub fn entry_address(&self) -> &EntryAddress {
        match &self.content {
            ChainHeaderContent::StubHeaderType { entry_address, .. } => {
                entry_address
            }
        }
    }

    /// Return the previous ChainHeader in the chain
    pub fn prev_header_address(&self) -> Option<&HeaderAddress> {
        match &self.content {
            ChainHeaderContent::StubHeaderType { prev_header_address, .. } => {
                prev_header_address.as_ref()
            }
        }
    }
}

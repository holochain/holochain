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

/// ChainHeader of a source chain "Item"
/// The address of the ChainHeader is used as the Item's key in the source chain hash table
/// ChainHeaders are linked to next header in chain and next header of same type in chain
// @TODO - serialize properties as defined in ChainHeadersEntrySchema from golang alpha 1
// @see https://github.com/holochain/holochain-proto/blob/4d1b8c8a926e79dfe8deaa7d759f930b66a5314f/entry_headers.go#L7
// @see https://github.com/holochain/holochain-rust/issues/75
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainHeader {}

impl ChainHeader {
    /// build a new ChainHeader from a chain, entry type and entry.
    /// a ChainHeader is immutable, but the chain is mutable if chain.push() is used.
    /// this means that a header becomes invalid and useless as soon as the chain is mutated
    /// the only valid usage of a header is to immediately push it onto a chain in a Pair.
    /// normally (outside unit tests) the generation of valid headers is internal to the
    /// chain::SourceChain trait and should not need to be handled manually
    ///
    /// @see chain::entry::Entry
    pub fn new() -> Self {
        ChainHeader {}
    }

    /// Return the EntryHash this header points to
    pub fn entry_address(&self) -> EntryAddress {
        todo!("return entry address")
    }

    /// Return the previous ChainHeader in the chain
    pub fn prev_header_address(&self) -> Option<HeaderAddress> {
        todo!("return previous header hash")
    }
}

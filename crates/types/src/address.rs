//! wraps holo_hashes for the use of those hashes as storage addresses, either CAS or DHT

use crate::{entry::Entry, ChainHeader};
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// address type for header hash to promote it to an "address" e.g. for use in a CAS
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

impl std::fmt::Display for HeaderAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HeaderAddress::Header(hash) => write!(f, "{}", hash),
        }
    }
}

/// address type for entry hashes that can be used to retrieve entries from the cas or dht
#[derive(
    Debug, Clone, derive_more::From, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub enum EntryAddress {
    /// standard entry hash
    Entry(EntryHash),
    /// agents are entries too
    Agent(AgentPubKey),
}

impl From<EntryAddress> for HoloHash {
    fn from(entry_address: EntryAddress) -> HoloHash {
        match entry_address {
            EntryAddress::Entry(entry_hash) => entry_hash.into(),
            EntryAddress::Agent(agent_pubkey) => agent_pubkey.into(),
        }
    }
}

impl TryFrom<&Entry> for EntryAddress {
    type Error = SerializedBytesError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(EntryAddress::Entry(EntryHash::try_from(entry)?))
    }
}

impl std::fmt::Display for EntryAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EntryAddress::Entry(entry_hash) => entry_hash.fmt(f),
            EntryAddress::Agent(agent_pubkey) => agent_pubkey.fmt(f),
        }
    }
}

/// address type for hashes that can be used to retrieve anything that can be stored on the dht
#[derive(Debug, Clone, derive_more::From, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DhtAddress {
    /// standard entry hash
    Entry(EntryHash),
    /// agents can be stored
    Agent(AgentPubKey),
    /// headers can be stored
    Header(HeaderHash),
}

impl From<DhtAddress> for HoloHash {
    fn from(entry_address: DhtAddress) -> HoloHash {
        match entry_address {
            DhtAddress::Entry(entry_hash) => entry_hash.into(),
            DhtAddress::Agent(agent_pubkey) => agent_pubkey.into(),
            DhtAddress::Header(header_hash) => header_hash.into(),
        }
    }
}

impl TryFrom<&Entry> for DhtAddress {
    type Error = SerializedBytesError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(DhtAddress::Entry(EntryHash::try_from(entry)?))
    }
}

impl TryFrom<&ChainHeader> for DhtAddress {
    type Error = SerializedBytesError;
    fn try_from(header: &ChainHeader) -> Result<Self, Self::Error> {
        Ok(DhtAddress::Header(HeaderHash::try_from(header)?))
    }
}

impl TryFrom<&AgentPubKey> for DhtAddress {
    type Error = SerializedBytesError;
    fn try_from(agent: &AgentPubKey) -> Result<Self, Self::Error> {
        Ok(DhtAddress::Agent(agent.to_owned()))
    }
}

impl std::fmt::Display for DhtAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DhtAddress::Entry(entry_hash) => write!(f, "{}", entry_hash),
            DhtAddress::Agent(agent_pubkey) => write!(f, "{}", agent_pubkey),
            DhtAddress::Header(header_hash) => write!(f, "{}", header_hash),
        }
    }
}

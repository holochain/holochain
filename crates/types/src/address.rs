//! wraps holo_hashes for the use of those hashes as storage addresses, either CAS or DHT

use crate::{chain_header::ChainHeader, entry::Entry};
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// address type for hashes that can be used to retrieve entries from the cas or dht
#[derive(Debug, Clone, derive_more::From, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryAddress {
    /// standard entry hash
    Entry(EntryHash),
    /// agents are entries too
    Agent(AgentHash),
}

impl From<EntryAddress> for HoloHash {
    fn from(entry_address: EntryAddress) -> HoloHash {
        match entry_address {
            EntryAddress::Entry(entry_hash) => entry_hash.into(),
            EntryAddress::Agent(agent_hash) => agent_hash.into(),
        }
    }
}

impl TryFrom<&Entry> for EntryAddress {
    type Error = SerializedBytesError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(EntryAddress::Entry(EntryHash::try_from(entry)?))
    }
}

impl AsRef<[u8]> for &EntryAddress {
    fn as_ref(&self) -> &[u8] {
        match self {
            EntryAddress::Entry(entry_hash) => entry_hash.as_ref(),
            EntryAddress::Agent(agent_hash) => agent_hash.as_ref(),
        }
    }
}

impl std::fmt::Display for EntryAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EntryAddress::Entry(entry_hash) => write!(f, "{}", entry_hash),
            EntryAddress::Agent(agent_hash) => write!(f, "{}", agent_hash),
        }
    }
}

/// address type for hashes that can be used to retrieve anything that can be stored on the dht
#[derive(Debug, Clone, derive_more::From, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DhtAddress {
    /// standard entry hash
    Entry(EntryHash),
    /// agents can be stored
    Agent(AgentHash),
    /// headers can be stored
    Header(HeaderHash),
}

impl From<DhtAddress> for HoloHash {
    fn from(entry_address: DhtAddress) -> HoloHash {
        match entry_address {
            DhtAddress::Entry(entry_hash) => entry_hash.into(),
            DhtAddress::Agent(agent_hash) => agent_hash.into(),
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

impl TryFrom<&AgentHash> for DhtAddress {
    type Error = SerializedBytesError;
    fn try_from(agent: &AgentHash) -> Result<Self, Self::Error> {
        Ok(DhtAddress::Agent(agent.to_owned()))
    }
}

impl AsRef<[u8]> for &DhtAddress {
    fn as_ref(&self) -> &[u8] {
        match self {
            DhtAddress::Entry(entry_hash) => entry_hash.as_ref(),
            DhtAddress::Agent(agent_hash) => agent_hash.as_ref(),
            DhtAddress::Header(header_hash) => header_hash.as_ref(),
        }
    }
}

impl std::fmt::Display for DhtAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DhtAddress::Entry(entry_hash) => write!(f, "{}", entry_hash),
            DhtAddress::Agent(agent_hash) => write!(f, "{}", agent_hash),
            DhtAddress::Header(header_hash) => write!(f, "{}", header_hash),
        }
    }
}

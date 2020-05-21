//! wraps holo_hashes for the use of those hashes as storage addresses, either CAS or DHT

use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// address type for header hash to promote it to an "address" e.g. for use when getting a header
/// from a CAS or the DHT.  This is similar to EntryAddress which promotes and entry hash to a
/// retrievable entity.
// TODO: Temporary alias, in case we decide again that we want to differentiate
// Address from Hash. Otherwise, this can be removed and all references of
// HeaderAddress can be renamed to HeaderHash
pub type HeaderAddress = HeaderHash;

/// address type for entry hashes that can be used to retrieve entries from the cas or dht
#[derive(
    Debug,
    Clone,
    derive_more::From,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    SerializedBytes,
)]
pub enum EntryAddress {
    /// standard entry hash
    Entry(EntryHash),
    /// agents are entries too
    Agent(AgentPubKey),
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_entry_addr {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            EntryAddress::Entry($i) => {
                $($t)*
            }
            EntryAddress::Agent($i) => {
                $($t)*
            }
        }
    };
}

impl holo_hash_core::HoloHashCoreHash for EntryAddress {
    fn get_raw(&self) -> &[u8] {
        match_entry_addr!(self => |i| { i.get_raw() })
    }

    fn get_bytes(&self) -> &[u8] {
        match_entry_addr!(self => |i| { i.get_bytes() })
    }

    fn get_loc(&self) -> u32 {
        match_entry_addr!(self => |i| { i.get_loc() })
    }

    fn into_inner(self) -> std::vec::Vec<u8> {
        match_entry_addr!(self => |i| { i.into_inner() })
    }
}

impl From<EntryAddress> for holo_hash_core::HoloHashCore {
    fn from(entry_hash: EntryAddress) -> holo_hash_core::HoloHashCore {
        match_entry_addr!(entry_hash => |i| { i.into() })
    }
}

impl From<EntryAddress> for HoloHash {
    fn from(entry_hash: EntryAddress) -> HoloHash {
        match_entry_addr!(entry_hash => |i| { i.into() })
    }
}

impl std::fmt::Display for EntryAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match_entry_addr!(self => |i| { i.fmt(f) })
    }
}

/// address type for hashes that can be used to retrieve anything that can be stored on the dht
#[derive(Debug, Clone, derive_more::From, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DhtAddress {
    /// standard entry content hash
    EntryContent(EntryHash),
    /// agents can be stored
    Agent(AgentPubKey),
    /// headers can be stored
    Header(HeaderHash),
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_dht_addr {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            DhtAddress::EntryContent($i) => {
                $($t)*
            }
            DhtAddress::Agent($i) => {
                $($t)*
            }
            DhtAddress::Header($i) => {
                $($t)*
            }
        }
    };
}

impl From<DhtAddress> for HoloHash {
    fn from(dht_address: DhtAddress) -> HoloHash {
        match_dht_addr!(dht_address => |i| { i.into() })
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
        match_dht_addr!(self => |i| { i.fmt(f) })
    }
}

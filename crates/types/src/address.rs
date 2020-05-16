//! wraps holo_hashes for the use of those hashes as storage addresses, either CAS or DHT

use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::Entry;

/// address type for header hash to promote it to an "address" e.g. for use when getting a header
/// from a CAS or the DHT.  This is similar to EntryAddress which promotes and entry hash to a
/// retrievable entity.
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
pub enum HeaderAddress {
    /// a header hash, the only option
    Header(HeaderHash),
}

/// a utility macro just to not have to type in the match statement everywhere.
macro_rules! match_header_addr {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            HeaderAddress::Header($i) => {
                $($t)*
            }
        }
    };
}

impl holo_hash_core::HoloHashCoreHash for HeaderAddress {
    fn get_raw(&self) -> &[u8] {
        match_header_addr!(self => |i| { i.get_raw() })
    }

    fn get_bytes(&self) -> &[u8] {
        match_header_addr!(self => |i| { i.get_bytes() })
    }

    fn get_loc(&self) -> u32 {
        match_header_addr!(self => |i| { i.get_loc() })
    }

    fn into_inner(self) -> std::vec::Vec<u8> {
        match_header_addr!(self => |i| { i.into_inner() })
    }
}

impl PartialEq<HeaderHash> for HeaderAddress {
    #[allow(irrefutable_let_patterns)]
    fn eq(&self, other: &HeaderHash) -> bool {
        if let HeaderAddress::Header(hash) = self {
            return hash == other;
        }
        false
    }
}

impl From<HeaderAddress> for holo_hash_core::HoloHashCore {
    fn from(header_address: HeaderAddress) -> holo_hash_core::HoloHashCore {
        match_header_addr!(header_address => |i| { i.into() })
    }
}

impl From<HeaderAddress> for HoloHash {
    fn from(header_address: HeaderAddress) -> HoloHash {
        match_header_addr!(header_address => |i| { i.into() })
    }
}

impl From<holo_hash::holo_hash_core::HeaderHash> for HeaderAddress {
    fn from(header_hash: holo_hash::holo_hash_core::HeaderHash) -> HeaderAddress {
        holo_hash::HeaderHash::from(header_hash).into()
    }
}

impl From<&HeaderHash> for HeaderAddress {
    fn from(header_hash: &HeaderHash) -> HeaderAddress {
        header_hash.to_owned().into()
    }
}

impl std::fmt::Display for HeaderAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match_header_addr!(self => |i| { i.fmt(f) })
    }
}

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

impl TryFrom<&Entry> for EntryAddress {
    type Error = SerializedBytesError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(match entry {
            Entry::Agent(key) => EntryAddress::Agent(key.to_owned().into()),
            _ => {
                let serialized_bytes: SerializedBytes = entry.try_into()?;
                EntryAddress::Entry(EntryHash::with_data_sync(serialized_bytes.bytes()))
            }
        })
    }
}

impl TryFrom<Entry> for EntryAddress {
    type Error = SerializedBytesError;
    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        Self::try_from(&entry)
impl From<EntryAddress> for holo_hash_core::HoloHashCore {
    fn from(entry_address: EntryAddress) -> holo_hash_core::HoloHashCore {
        match_entry_addr!(entry_address => |i| { i.into() })
    }
}

impl From<EntryAddress> for HoloHash {
    fn from(entry_address: EntryAddress) -> HoloHash {
        match_entry_addr!(entry_address => |i| { i.into() })
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
    /// standard entry hash
    Entry(EntryHash),
    /// agents can be stored
    Agent(AgentPubKey),
    /// headers can be stored
    Header(HeaderHash),
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_dht_addr {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            DhtAddress::Entry($i) => {
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

impl TryFrom<&Entry> for DhtAddress {
    type Error = SerializedBytesError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        Ok(match EntryAddress::try_from(entry)? {
            EntryAddress::Entry(entry_hash) => DhtAddress::Entry(entry_hash),
            EntryAddress::Agent(agent_pub_key) => DhtAddress::Agent(agent_pub_key),
        })
    }
}

impl TryFrom<&Header> for DhtAddress {
    type Error = SerializedBytesError;
    fn try_from(header: &Header) -> Result<Self, Self::Error> {
        Ok(DhtAddress::Header(HeaderHash::try_from(header)?))
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

//! wraps holo_hashes for the use of those hashes as storage addresses, either CAS or DHT

use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// address type for header hash to promote it to an "address" e.g. for use when getting a header
/// from a CAS or the DHT.  This is similar to EntryHash which promotes and entry hash to a
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
pub enum EntryHash {
    /// standard entry hash
    Entry(EntryContentHash),
    /// agents are entries too
    Agent(AgentPubKey),
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_entry_hash {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            EntryHash::Entry($i) => {
                $($t)*
            }
            EntryHash::Agent($i) => {
                $($t)*
            }
        }
    };
}

impl holo_hash_core::HoloHashCoreHash for EntryHash {
    fn get_raw(&self) -> &[u8] {
        match_entry_hash!(self => |i| { i.get_raw() })
    }

    fn get_bytes(&self) -> &[u8] {
        match_entry_hash!(self => |i| { i.get_bytes() })
    }

    fn get_loc(&self) -> u32 {
        match_entry_hash!(self => |i| { i.get_loc() })
    }

    fn into_inner(self) -> std::vec::Vec<u8> {
        match_entry_hash!(self => |i| { i.into_inner() })
    }
}

impl From<EntryHash> for holo_hash_core::HoloHashCore {
    fn from(entry_hash: EntryHash) -> holo_hash_core::HoloHashCore {
        match_entry_hash!(entry_hash => |i| { i.into() })
    }
}

impl TryFrom<holo_hash_core::HoloHashCore> for EntryHash {
    type Error = HoloHashError;
    fn try_from(holo_hash: holo_hash_core::HoloHashCore) -> Result<Self, Self::Error> {
        Ok(match holo_hash {
            holo_hash_core::HoloHashCore::AgentPubKey(v) => EntryHash::Agent(v.into()),
            holo_hash_core::HoloHashCore::EntryContentHash(v) => EntryHash::Entry(v.into()),
            _ => Err(HoloHashError::BadPrefix)?,
        })
    }
}

impl From<EntryHash> for HoloHash {
    fn from(entry_hash: EntryHash) -> HoloHash {
        match_entry_hash!(entry_hash => |i| { i.into() })
    }
}

impl From<EntryHash> for AnyDhtHash {
    fn from(entry_hash: EntryHash) -> AnyDhtHash {
        match_entry_hash!(entry_hash => |i| { i.into() })
    }
}

impl std::fmt::Display for EntryHash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match_entry_hash!(self => |i| { i.fmt(f) })
    }
}

impl AsRef<[u8]> for EntryHash {
    fn as_ref(&self) -> &[u8] {
        self.get_bytes()
    }
}

/// address type for hashes that can be used to retrieve anything that can be stored on the dht
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
pub enum AnyDhtHash {
    /// standard entry content hash
    EntryContent(EntryContentHash),
    /// agents can be stored
    Agent(AgentPubKey),
    /// headers can be stored
    Header(HeaderHash),
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_dht_hash {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            AnyDhtHash::EntryContent($i) => {
                $($t)*
            }
            AnyDhtHash::Agent($i) => {
                $($t)*
            }
            AnyDhtHash::Header($i) => {
                $($t)*
            }
        }
    };
}

impl holo_hash_core::HoloHashCoreHash for AnyDhtHash {
    fn get_raw(&self) -> &[u8] {
        match_dht_hash!(self => |i| { i.get_raw() })
    }

    fn get_bytes(&self) -> &[u8] {
        match_dht_hash!(self => |i| { i.get_bytes() })
    }

    fn get_loc(&self) -> u32 {
        match_dht_hash!(self => |i| { i.get_loc() })
    }

    fn into_inner(self) -> std::vec::Vec<u8> {
        match_dht_hash!(self => |i| { i.into_inner() })
    }
}

impl From<AnyDhtHash> for holo_hash_core::HoloHashCore {
    fn from(any_dht_hash: AnyDhtHash) -> holo_hash_core::HoloHashCore {
        match_dht_hash!(any_dht_hash => |i| { i.into() })
    }
}

impl From<AnyDhtHash> for HoloHash {
    fn from(any_dht_hash: AnyDhtHash) -> HoloHash {
        match_dht_hash!(any_dht_hash => |i| { i.into() })
    }
}

impl std::fmt::Display for AnyDhtHash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match_dht_hash!(self => |i| { i.fmt(f) })
    }
}

impl AsRef<[u8]> for AnyDhtHash {
    fn as_ref(&self) -> &[u8] {
        match self {
            AnyDhtHash::EntryContent(h) => h.as_ref(),
            AnyDhtHash::Agent(h) => h.as_ref(),
            AnyDhtHash::Header(h) => h.as_ref(),
        }
    }
}

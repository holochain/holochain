//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::dna::Dna;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// Structure holding actual data in a source chain "Item"
/// data is stored as a JsonString
#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq, SerializedBytes)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "entry_type", content = "entry")]
pub enum Entry {
    /// The Dna system entry, the first entry of every source chain
    Dna(Box<Dna>),
    /// The AgentKey system entry, the second entry of every source chain,
    /// which grants authoring capability for this agent. (Name TBD)
    AgentKey(AgentHash),
}

impl Entry {
    /// Get the EntryHash of this entry
    pub fn entry_hash(&self) -> EntryHash {
        let sb: SerializedBytes = self.try_into().expect("TODO: can this fail?");
        EntryHash::with_data_sync(&sb.bytes())
    }
}

/// wraps hashes that can be used as addresses for entries e.g. in a CAS
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

// // TEST UTILS START
// // @TODO - move all of this into some kind of nice fixtures setup

// #[derive(Serialize, Deserialize, SerializedBytes)]
// struct SerializedString(String);

// /// dummy entry value
// #[cfg_attr(tarpaulin, skip)]
// pub fn test_entry_value() -> SerializedBytes {
//     SerializedBytes::try_from(()).unwrap()
// }

// /// dummy entry
// #[cfg_attr(tarpaulin, skip)]
// pub fn test_entry() -> Entry {
//     Entry::App(
//         crate::entry::entry_type::test_app_entry_type(),
//         test_entry_value(),
//     )
// }

// /// dummy entry, same as test_entry()
// #[cfg_attr(tarpaulin, skip)]
// pub fn test_entry_a() -> Entry {
//     test_entry()
// }

// /// dummy entry, differs from test_entry()
// #[cfg_attr(tarpaulin, skip)]
// pub fn test_entry_b() -> Entry {
//     Entry::App(test_app_entry_type_b(), test_entry_value_b())
// }

// /// dummy entry content, differs from test_entry_value()
// #[cfg_attr(tarpaulin, skip)]
// pub fn test_entry_value_b() -> SerializedBytes {
//     SerializedBytes::try_from(SerializedString(String::from("other test entry value"))).unwrap()
// }
// #[cfg_attr(tarpaulin, skip)]
// /// dummy entry content c
// pub fn test_entry_value_c() -> SerializedBytes {
//     SerializedBytes::try_from(SerializedString(String::from("value C"))).unwrap()
// }
// /// dummy entry c
// pub fn test_entry_c() -> Entry {
//     Entry::App(test_app_entry_type_b(), test_entry_value_c())
// }

// #[cfg(test)]
// pub fn test_entry_hash() -> holo_hash::EntryHash {
//     holo_hash::EntryHash::try_from(crate::entry::test_entry()).unwrap()
// }
// #[cfg(test)]
// pub fn test_entry_address() -> EntryAddress {
//     EntryAddress::Entry(test_entry_hash())
// }
// #[cfg(test)]
// pub fn test_entry_hash_b() -> holo_hash::EntryHash {
//     holo_hash::EntryHash::try_from(crate::entry::test_entry_b()).unwrap()
// }
// #[cfg(test)]
// pub fn test_entry_address_b() -> EntryAddress {
//     EntryAddress::Entry(test_entry_hash_b())
// }
// #[cfg(test)]
// pub fn test_entry_hash_c() -> holo_hash::EntryHash {
//     holo_hash::EntryHash::try_from(crate::entry::test_entry_c()).unwrap()
// }
// #[cfg(test)]
// pub fn test_entry_address_c() -> EntryAddress {
//     EntryAddress::Entry(test_entry_hash_c())
// }

// #[cfg(test)]
// pub mod tests {

//     use crate::agent::AgentId;
//     use crate::entry::test_entry;
//     use crate::entry::test_entry_a;
//     use crate::entry::test_entry_b;
//     use crate::entry::test_entry_value;
//     use crate::entry::Entry;
//     use crate::entry::SerializedString;
//     use crate::{
//         agent::test_agent_id, entry::entry_type::test_app_entry_type,
//         persistence::cas::content::Address, persistence::cas::content::Addressable,
//         test_utils::fake_dna,
//     };
//     use holochain_serialized_bytes::prelude::*;

//     pub fn test_entry_content() -> SerializedBytes {
//         SerializedBytes::try_from(Entry::App(test_app_entry_type(), test_entry_value())).unwrap()
//     }

//     /// dummy entry content, same as test_entry_value()
//     #[cfg_attr(tarpaulin, skip)]
//     pub fn test_entry_value_a() -> SerializedBytes {
//         test_entry_value()
//     }

//     #[cfg_attr(tarpaulin, skip)]
//     pub fn test_sys_entry_value() -> AgentId {
//         test_agent_id()
//     }

//     #[cfg_attr(tarpaulin, skip)]
//     pub fn test_entry_with_value<T: TryInto<SerializedBytes>>(value: T) -> Entry
//     where
//         <T as TryInto<SerializedBytes>>::Error: std::fmt::Debug,
//     {
//         Entry::App(test_app_entry_type(), value.try_into().unwrap())
//     }

//     pub fn expected_serialized_entry_content() -> SerializedBytes {
//         SerializedBytes::try_from(test_entry()).unwrap()
//     }

//     /// the correct address for test_entry()
//     #[cfg_attr(tarpaulin, skip)]
//     pub fn expected_entry_address() -> Address {
//         Address::from("QmYd5fc7jzVZAQRuYGKU5PAiXeWoUEEaH4ogJyHR1RbQGw".to_string())
//     }

//     /// dummy entry with unique string content
//     #[cfg_attr(tarpaulin, skip)]
//     pub fn test_entry_unique() -> Entry {
//         Entry::App(
//             test_app_entry_type(),
//             SerializedString(snowflake::ProcessUniqueId::new().to_string())
//                 .try_into()
//                 .unwrap(),
//         )
//     }

//     #[cfg_attr(tarpaulin, skip)]
//     pub fn test_sys_entry() -> Entry {
//         Entry::AgentId(test_sys_entry_value())
//     }

//     pub fn test_sys_entry_address() -> Address {
//         Address::from(String::from(
//             "QmUZ3wsC4sVdJZK2AC8Ji4HZRfkFSH2cYE6FntmfWKF8GV",
//         ))
//     }

//     #[cfg_attr(tarpaulin, skip)]
//     pub fn test_unpublishable_entry() -> Entry {
//         Entry::Dna(Box::new(fake_dna("uuid")))
//     }

//     #[test]
//     /// tests for PartialEq
//     fn eq() {
//         let entry_a = test_entry_a();
//         let entry_b = test_entry_b();

//         // same content is equal
//         assert_eq!(entry_a, entry_a);

//         // different content is not equal
//         assert_ne!(entry_a, entry_b);
//     }

//     #[test]
//     /// test entry.address() against a known value
//     fn known_address() {
//         assert_eq!(expected_entry_address(), test_entry().address());
//     }
// }

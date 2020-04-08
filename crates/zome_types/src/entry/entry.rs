//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::address::Address;
use crate::{
    agent::AgentId,
    dna::Dna,
    entry::{
        cap_entries::{CapTokenClaim, CapTokenGrant},
        deletion_entry::DeletionEntry,
        entry_type::{AppEntryType, EntryType},
    },
    link::Link,
};
use holochain_serialized_bytes::prelude::*;

#[cfg(test)]
use sx_fixture::Fixture;
#[cfg(test)]
use sx_fixture::FixtureType;

pub enum EntryError {
    /// Attempted to convert any EntryType other than App to AppEntryType
    AppEntryTypeConversion(EntryType),
}

/// Should probably be a newtype.
pub type AppEntryValue = SerializedBytes;

/// Structure holding actual data in a source chain "Item"
/// data is stored as a JsonString
#[derive(Clone, Debug, Serialize, Deserialize, Eq, SerializedBytes)]
#[allow(clippy::large_enum_variant)]
pub enum Entry {
    /// An App (user defined) Entry
    App(AppEntryType, AppEntryValue),

    /// The DNA entry defines the rules for an application.
    Dna(Box<Dna>),

    /// The AgentId entry defines who has agency over the source chain.
    AgentId(AgentId),

    /// A deletion entry.
    Deletion(DeletionEntry),

    /// Create a link entry.
    LinkAdd(Link),

    /// Mark a link as removed (though the add entry will persist).
    LinkRemove((Link, Vec<Address>)),

    // ChainHeader(ChainHeader),
    // ChainMigrate(ChainMigrate),
    /// Claim a capability.
    CapTokenClaim(CapTokenClaim),

    /// Grant a capability.
    CapTokenGrant(CapTokenGrant),
}

impl Entry {
    /// Get the type of this entry.
    pub fn entry_type(&self) -> EntryType {
        match &self {
            Entry::App(app_entry_type, _) => EntryType::App(app_entry_type.to_owned()),
            Entry::Dna(_) => EntryType::Dna,
            Entry::AgentId(_) => EntryType::AgentId,
            Entry::Deletion(_) => EntryType::Deletion,
            Entry::LinkAdd(_) => EntryType::LinkAdd,
            Entry::LinkRemove(_) => EntryType::LinkRemove,
            // Entry::LinkList(_) => EntryType::LinkList,
            // Entry::ChainHeader(_) => EntryType::ChainHeader,
            // Entry::ChainMigrate(_) => EntryType::ChainMigrate,
            Entry::CapTokenClaim(_) => EntryType::CapTokenClaim,
            Entry::CapTokenGrant(_) => EntryType::CapTokenGrant,
        }
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Entry) -> bool {
        SerializedBytes::try_from(self).unwrap() == SerializedBytes::try_from(other).unwrap()
    }
}

/// The address of an entry.
pub struct EntryAddress(Address);

#[cfg(test)]
pub enum EntryFixtureType {
    App,
    Dna,
}

#[cfg(test)]
impl Fixture for Entry {
    type Input = EntryFixtureType;
    fn fixture(fixture_type: FixtureType<Self::Input>) -> Self {
        match fixture_type {
            FixtureType::A => Entry::App(
                AppEntryType::from("foo".to_string()),
                SerializedBytes::try_from(()).unwrap(),
            ),
            FixtureType::FromInput(entry_fixture_type) => {
                match entry_fixture_type {
                    EntryFixtureType::App => unimplemented!(),
                    EntryFixtureType::Dna => Entry::Dna(Box::new(Dna::fixture(FixtureType::A))),
                }
            },
            _ => unimplemented!(),
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::address::Addressable;
    use sx_fixture::Fixture;
    use sx_fixture::FixtureType;

    #[derive(Serialize, Deserialize, SerializedBytes)]
    struct SerializedString(String);

    #[test]
    /// tests for PartialEq
    fn eq() {
        let entry_a = Entry::fixture(FixtureType::A);
        let entry_b = Entry::fixture(FixtureType::B);

        // same content is equal
        assert_eq!(entry_a, entry_a);

        // different content is not equal
        assert_ne!(entry_a, entry_b);
    }

    #[test]
    /// test entry.address() against a known value
    fn known_address() {
        assert_eq!(Address::new(vec![0]), Entry::fixture(FixtureType::A).address());
    }
}

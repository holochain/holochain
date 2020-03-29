//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::{
    agent::AgentId,
    dna::Dna,
    entry::{
        cap_entries::{CapTokenClaim, CapTokenGrant},
        deletion_entry::DeletionEntry,
        entry_type::{AppEntryType, EntryType},
    },
    link::Link,
    persistence::cas::content::{Address, Addressable, Content},
    prelude::*,
};
use holochain_serialized_bytes::prelude::*;
use multihash::Hash;

/// Should probably be a newtype.
pub type AppEntryValue = SerializedBytes;

/// Structure holding actual data in a source chain "Item"
/// data is stored as a JsonString
#[derive(Clone, Debug, Serialize, Deserialize, Eq)]
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
        self.address() == other.address()
    }
}

impl Addressable for Entry {
    fn address(&self) -> Address {
        match &self {
            Entry::AgentId(agent_id) => agent_id.address(),
            // @TODO deal with unwrap here
            _ => {
                Address::encode_from_bytes(Content::try_from(self).unwrap().bytes(), Hash::SHA2256)
            }
        }
    }
}

/// The address of an entry.
pub struct EntryAddress(Address);

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::{
        agent::test_agent_id,
        entry::entry_type::tests::{test_app_entry_type, test_app_entry_type_b},
    };

    use crate::persistence::cas::content::{AddressableContent, AddressableContentTestSuite};

    /// dummy entry value
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_value() -> JsonString {
        JsonString::from(RawString::from("test entry value"))
    }

    pub fn test_entry_content() -> Content {
        Content::from("{\"App\":[\"testEntryType\",\"\\\"test entry value\\\"\"]}")
    }

    /// dummy entry content, same as test_entry_value()
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_value_a() -> JsonString {
        test_entry_value()
    }

    /// dummy entry content, differs from test_entry_value()
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_value_b() -> JsonString {
        JsonString::from(RawString::from("other test entry value"))
    }
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_value_c() -> JsonString {
        RawString::from("value C").into()
    }

    #[cfg_attr(tarpaulin, skip)]
    pub fn test_sys_entry_value() -> AgentId {
        test_agent_id()
    }

    /// dummy entry
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry() -> Entry {
        Entry::App(test_app_entry_type(), test_entry_value())
    }
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_with_value(value: &'static str) -> Entry {
        Entry::App(test_app_entry_type(), JsonString::from_json(&value))
    }

    pub fn expected_serialized_entry_content() -> JsonString {
        JsonString::from_json("{\"App\":[\"testEntryType\",\"\\\"test entry value\\\"\"]}")
    }

    /// the correct address for test_entry()
    #[cfg_attr(tarpaulin, skip)]
    pub fn expected_entry_address() -> Address {
        Address::from("Qma6RfzvZRL127UCEVEktPhQ7YSS1inxEFw7SjEsfMJcrq".to_string())
    }

    /// dummy entry, same as test_entry()
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_a() -> Entry {
        test_entry()
    }

    /// dummy entry, differs from test_entry()
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_b() -> Entry {
        Entry::App(test_app_entry_type_b(), test_entry_value_b())
    }
    pub fn test_entry_c() -> Entry {
        Entry::App(test_app_entry_type_b(), test_entry_value_c())
    }

    /// dummy entry with unique string content
    #[cfg_attr(tarpaulin, skip)]
    pub fn test_entry_unique() -> Entry {
        Entry::App(
            test_app_entry_type(),
            RawString::from(snowflake::ProcessUniqueId::new().to_string()).into(),
        )
    }

    #[cfg_attr(tarpaulin, skip)]
    pub fn test_sys_entry() -> Entry {
        Entry::AgentId(test_sys_entry_value())
    }

    pub fn test_sys_entry_address() -> Address {
        Address::from(String::from(
            "QmUZ3wsC4sVdJZK2AC8Ji4HZRfkFSH2cYE6FntmfWKF8GV",
        ))
    }

    #[cfg_attr(tarpaulin, skip)]
    pub fn test_unpublishable_entry() -> Entry {
        Entry::Dna(Box::new(Dna::empty()))
    }

    #[test]
    /// tests for PartialEq
    fn eq() {
        let entry_a = test_entry_a();
        let entry_b = test_entry_b();

        // same content is equal
        assert_eq!(entry_a, entry_a);

        // different content is not equal
        assert_ne!(entry_a, entry_b);
    }

    #[test]
    /// test entry.address() against a known value
    fn known_address() {
        assert_eq!(expected_entry_address(), test_entry().address());
    }

    #[test]
    /// show From<Entry> for JsonString
    fn json_string_from_entry_test() {
        assert_eq!(
            test_entry().content(),
            JsonString::from(Entry::from(test_entry()))
        );
    }

    #[test]
    /// show From<Content> for Entry
    fn entry_from_content_test() {
        assert_eq!(
            test_entry(),
            Entry::try_from(test_entry().content()).unwrap()
        );
    }

    #[test]
    /// tests for entry.content()
    fn content_test() {
        let content = test_entry_content();
        let entry = Entry::try_from_content(&content).unwrap();

        assert_eq!(content, entry.content());
    }

    #[test]
    /// test that we can round trip through JSON
    fn json_round_trip() {
        let entry = test_entry();
        let expected = expected_serialized_entry_content();
        assert_eq!(expected, JsonString::from(Entry::from(entry.clone())));
        assert_eq!(entry, Entry::try_from(expected.clone()).unwrap());
        assert_eq!(entry, Entry::from(entry.clone()));

        let sys_entry = test_sys_entry();
        let expected = JsonString::from_json(&format!(
            "{{\"AgentId\":{{\"nick\":\"{}\",\"pub_sign_key\":\"{}\"}}}}",
            "bob",
            crate::agent::GOOD_ID,
        ));
        assert_eq!(expected, JsonString::from(Entry::from(sys_entry.clone())));
        assert_eq!(
            &sys_entry,
            &Entry::from(Entry::try_from(expected.clone()).unwrap())
        );
        assert_eq!(&sys_entry, &Entry::from(Entry::from(sys_entry.clone())),);
    }

    #[test]
    /// show AddressableContent implementation
    fn addressable_content_test() {
        // from_content()
        AddressableContentTestSuite::addressable_content_trait_test::<Entry>(
            test_entry_content(),
            test_entry(),
            expected_entry_address(),
        );
    }
}

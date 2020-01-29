//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::agent::AgentId;
use crate::entry::entry_type::AppEntryType;
use crate::entry::entry_type::EntryType;
use crate::entry::{
    cap_entries::{CapTokenClaim, CapTokenGrant},
    deletion_entry::DeletionEntry,
};
use crate::link::Link;
use crate::shims::Dna;
use holochain_json_api::error::JsonResult;
use holochain_persistence_api::cas::content::Address;
use holochain_persistence_api::cas::content::AddressableContent;
use holochain_persistence_api::cas::content::Content;
// use crate::shims::*;
use holochain_json_api::{
    error::JsonError,
    json::{JsonString, RawString},
};
use multihash::Hash;
use serde::{ser::SerializeTuple, Deserialize, Deserializer, Serializer};
use snowflake;
use std::convert::TryFrom;

pub type AppEntryValue = JsonString;

fn serialize_app_entry<S>(
    app_entry_type: &AppEntryType,
    app_entry_value: &AppEntryValue,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut state = serializer.serialize_tuple(2)?;
    state.serialize_element(&app_entry_type.to_string())?;
    state.serialize_element(&app_entry_value.to_string())?;
    state.end()
}

fn deserialize_app_entry<'de, D>(deserializer: D) -> Result<(AppEntryType, AppEntryValue), D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct SerializedAppEntry(String, String);

    let serialized_app_entry = SerializedAppEntry::deserialize(deserializer)?;
    Ok((
        AppEntryType::from(serialized_app_entry.0),
        AppEntryValue::from_json(&serialized_app_entry.1),
    ))
}

/// Structure holding actual data in a source chain "Item"
/// data is stored as a JsonString
#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Entry {
    #[serde(serialize_with = "serialize_app_entry")]
    #[serde(deserialize_with = "deserialize_app_entry")]
    App(AppEntryType, AppEntryValue),

    Dna(Box<Dna>),
    AgentId(AgentId),
    Deletion(DeletionEntry),
    LinkAdd(Link),
    LinkRemove((Link, Vec<Address>)),
    // ChainHeader(ChainHeader),
    // ChainMigrate(ChainMigrate),
    CapTokenClaim(CapTokenClaim),
    CapTokenGrant(CapTokenGrant),
}

impl Entry {
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

impl AddressableContent for Entry {
    fn address(&self) -> Address {
        match &self {
            Entry::AgentId(agent_id) => agent_id.address(),
            _ => Address::encode_from_str(&String::from(self.content()), Hash::SHA2256),
        }
    }

    fn content(&self) -> Content {
        match &self {
            // Entry::ChainHeader(chain_header) => chain_header.into(),
            _ => self.into(),
        }
    }

    fn try_from_content(content: &Content) -> JsonResult<Entry> {
        Entry::try_from(content.to_owned())
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::agent::test_agent_id;
    use crate::entry::entry_type::tests::test_app_entry_type;
    use crate::entry::entry_type::tests::test_app_entry_type_b;
    use crate::prelude::*;
    use holochain_persistence_api::cas::{
        content::{AddressableContent, AddressableContentTestSuite},
        storage::{test_content_addressable_storage, ExampleContentAddressableStorage},
    };

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
        Entry::Dna(Box::new(Dna::new()))
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

    #[test]
    /// show CAS round trip
    fn cas_round_trip_test() {
        let entries = vec![test_entry()];
        AddressableContentTestSuite::addressable_content_round_trip::<
            Entry,
            ExampleContentAddressableStorage,
        >(entries, test_content_addressable_storage());
    }
}

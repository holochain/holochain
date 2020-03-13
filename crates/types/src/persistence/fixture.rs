use crate::persistence::{
    cas::content::{Address, AddressableContent, Content},
    hash::HashString,
};
use holochain_json_api::{
    error::{JsonError, JsonResult},
    json::JsonString,
};
use std::convert::TryFrom;

/// dummy hash based on the key of test_entry_a()
pub fn test_hash_a() -> HashString {
    test_entry_a().address()
}

pub fn test_entry_a() -> ExampleEntry {
    ExampleEntry::new(String::from("a"))
}

pub fn test_entry_b() -> ExampleEntry {
    ExampleEntry::new(String::from("b"))
}

pub fn test_eav_entity() -> ExampleEntry {
    test_entry_a()
}

pub fn test_eav_value() -> ExampleEntry {
    test_entry_b()
}

#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson)]
pub struct ExampleEntry {
    pub data: String,
}

impl AddressableContent for ExampleEntry {
    fn address(&self) -> Address {
        Address::from(self.data.clone())
    }

    fn content(&self) -> Content {
        self.into()
    }

    fn try_from_content(content: &Content) -> JsonResult<ExampleEntry> {
        ExampleEntry::try_from(content.to_owned())
    }
}

impl ExampleEntry {
    pub fn new(data: String) -> Self {
        Self { data }
    }
}

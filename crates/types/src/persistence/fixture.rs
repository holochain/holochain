//! helper functions and data for persistence testing

use crate::persistence::{cas::content::Addressable, hash::HashString};
use holochain_serialized_bytes::prelude::*;

/// dummy hash based on the key of test_entry_a()
pub fn test_hash_a() -> HashString {
    SerializedBytes::try_from(test_entry_a()).unwrap().address()
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExampleEntry {
    pub data: String,
}

holochain_serial!(ExampleEntry);

impl ExampleEntry {
    pub fn new(data: String) -> Self {
        Self { data }
    }
}

use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct Something(#[serde(with = "serde_bytes")] pub Vec<u8>);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Something(Something),
}

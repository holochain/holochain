use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct Something(#[serde(with = "serde_bytes")] pub Vec<u8>);

#[hdk_entry_defs]
pub enum EntryTypes {
    Something(Something),
}

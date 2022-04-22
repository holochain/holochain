use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub struct Thing;

#[hdk_entry_defs]
pub enum EntryTypes {
    Thing(Thing),
}

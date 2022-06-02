use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub struct Val(pub u32);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Post(Val),
}

use holochain_deterministic_integrity::prelude::*;

/// a tree of counters
#[hdk_entry_helper]
#[derive(Default, Clone, Copy, PartialEq)]
pub struct CounTree(pub u32);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Countree(CounTree),
}

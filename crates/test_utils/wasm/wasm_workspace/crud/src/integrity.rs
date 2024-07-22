use hdi::prelude::*;

/// a tree of counters
#[hdi_entry_helper]
#[derive(Default, Clone, Copy, PartialEq)]
pub struct CounTree(pub u32);

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Countree(CounTree),
}

use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Val(pub u32);

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Post(Val),
}

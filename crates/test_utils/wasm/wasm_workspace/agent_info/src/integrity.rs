use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Thing;

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Thing(Thing),
}

use hdi::prelude::*;

#[hdi_entry_helper]
pub struct Thing;

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Thing(Thing),
}

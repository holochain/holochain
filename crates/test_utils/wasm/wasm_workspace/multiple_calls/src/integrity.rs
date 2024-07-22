use hdi::prelude::*;

#[hdi_entry_helper]
pub struct Val(pub u32);

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Post(Val),
}

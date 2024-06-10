use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Test;

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Test(Test),
}

#[hdk_link_types]
pub enum LinkTypes {
    SomeLinks,
    SomeOtherLinks,
}

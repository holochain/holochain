use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct Test(String);

#[hdk_link_types]
pub enum LinkTypes {
    MyLink,
}

#[hdk_entry_defs]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    Test(Test),
}

pub const ANCHOR: &'static str = "init-anchor";

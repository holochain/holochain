use hdi::prelude::*;

#[hdk_entry_helper]
pub struct MyType {
    pub value: String,
}

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    MyType(MyType),
}

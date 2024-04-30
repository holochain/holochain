use hdi::prelude::*;

#[hdk_entry_helper]
pub struct MyType {
    pub value: String,
    pub amount: u32, // A difference from the original
}

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    MyType(MyType),
}

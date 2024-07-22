use hdi::prelude::*;

#[hdi_entry_helper]
pub struct MyOldType {
    pub value: String,
}

#[hdi_entry_helper]
pub struct MyType {
    pub value: String,
    pub amount: u32, // A difference from the original
}

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    MyType(MyType),
}

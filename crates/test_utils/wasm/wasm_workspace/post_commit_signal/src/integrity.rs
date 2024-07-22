use hdk::prelude::*;

#[hdi_entry_helper]
pub struct TestEntry(pub String);

#[derive(Serialize, Deserialize)]
#[hdi_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    TestEntry(TestEntry),
}

use hdk::prelude::*;

#[hdk_entry_helper]
pub struct TestEntry(pub String);

#[derive(Serialize, Deserialize)]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    TestEntry(TestEntry),
}

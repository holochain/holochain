use hdk::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct EmptyEntry;

#[derive(Serialize, Deserialize)]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    EmptyEntry(EmptyEntry),
}

#[hdk_extern]
fn validate(_: Op) -> ExternResult<usize> {
    Ok(42)
}

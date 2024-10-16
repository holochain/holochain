use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
pub fn create_entry_to_validate() -> ExternResult<ActionHash> {
    let details = EmptyEntry;
    create_entry(EntryTypes::EmptyEntry(details))
}

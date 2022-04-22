use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
pub fn assert_indexes(_: ()) -> ExternResult<()> {
    assert_eq!(EntryDefIndex(0), EntryTypes::Post(Post).entry_def_index());
    assert_eq!(
        EntryDefIndex(1),
        EntryTypes::Comment(Comment).entry_def_index()
    );
    Ok(())
}

#[hdk_extern]
pub fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk::prelude::zome_info()
}

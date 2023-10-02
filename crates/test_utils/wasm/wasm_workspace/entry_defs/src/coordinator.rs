use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
pub fn assert_indexes(_: ()) -> ExternResult<()> {
    // Note that this only works if there is a single integrity zome.
    assert_eq!(
        EntryDefIndex(0),
        ScopedEntryDefIndex::try_from(EntryTypes::Post(Post))?.zome_type
    );
    assert_eq!(
        EntryDefIndex(1),
        ScopedEntryDefIndex::try_from(EntryTypes::Comment(Comment))?.zome_type
    );
    Ok(())
}

#[hdk_extern]
pub fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk::prelude::zome_info()
}

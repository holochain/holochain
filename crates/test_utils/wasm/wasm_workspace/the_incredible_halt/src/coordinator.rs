use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
fn smash(_: ()) -> ExternResult<()> {
    loop {}
}

#[hdk_extern]
fn create_a_thing(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&EntryTypes::Thing(Thing))
}
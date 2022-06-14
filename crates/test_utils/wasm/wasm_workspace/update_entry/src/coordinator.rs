use crate::integrity::*;
use hdk::prelude::*;
use EntryZomes::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityUpdateEntry(EntryTypes),
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_entry(&IntegrityUpdateEntry(post()))
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<Option<Element>> {
    get(hash_entry(&post())?, GetOptions::latest())
}

#[hdk_extern]
fn update_entry(_: ()) -> ExternResult<HeaderHash> {
    let header_hash = hdk::prelude::create_entry(&IntegrityUpdateEntry(post()))?;
    hdk::prelude::update_entry(header_hash, &post())
}

#[hdk_extern]
/// Updates to a different entry, this will fail
fn invalid_update_entry(_: ()) -> ExternResult<HeaderHash> {
    let header_hash = hdk::prelude::create_entry(&IntegrityUpdateEntry(post()))?;
    hdk::prelude::update_entry(header_hash, &msg())
}
